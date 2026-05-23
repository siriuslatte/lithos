import {
  parse as parseYaml,
  parseDocument,
  isMap,
  isSeq,
  isScalar,
  isPair,
} from 'yaml';
import type { Plugin } from 'unified';
import { visit } from 'unist-util-visit';

export interface TransformLithosConfigExamplesOptions {
  mode?: 'all' | 'project';
}

const PROJECT_CONFIG_KEYS = new Set([
  'owner',
  'payments',
  'environments',
  'target',
  'state',
]);

// Reserved words that cannot be used as bare identifiers in Lua / Luau.
const LUAU_RESERVED = new Set([
  'and', 'break', 'do', 'else', 'elseif', 'end', 'false', 'for',
  'function', 'if', 'in', 'local', 'nil', 'not', 'or', 'repeat',
  'return', 'then', 'true', 'until', 'while', 'continue',
]);

type CodeNode = {
  lang?: string;
  meta?: string;
  value: string;
};

type PathSegment = string | number;
type Path = PathSegment[];

type YamlInfo = {
  value: unknown;
  lineToPath: Map<number, Path>;
};

type BuilderOutput = {
  text: string;
  pathToLine: Map<string, number>;
};

type Highlight = {
  raw: string;
  lines: Set<number>;
};

export function createTransformLithosConfigExamples(
  options: TransformLithosConfigExamplesOptions = {}
): Plugin<[]> {
  const mode = options.mode ?? 'project';

  return function () {
    return (tree: any) => {
      visit(tree, 'code', (node: any, index, parent: any) => {
        if (typeof index !== 'number' || !parent) {
          return;
        }

        const codeNode = node as CodeNode;
        normalizeCodeNode(codeNode);

        if (!isYamlCodeBlock(codeNode) || !shouldTransform(codeNode, mode)) {
          return;
        }

        const yamlInfo = parseYamlWithLineMap(codeNode.value);
        if (yamlInfo === undefined) {
          return;
        }

        const jsonOutput = buildJsonWithPathLines(yamlInfo.value);
        const luauOutput = buildLuauWithPathLines(yamlInfo.value);

        parent.children.splice(
          index,
          1,
          createTabsNode(codeNode, yamlInfo, jsonOutput, luauOutput)
        );
      });
    };
  };
}

function normalizeCodeNode(codeNode: CodeNode) {
  if (codeNode.lang === 'yml') {
    codeNode.lang = 'yaml';
  }

  if (codeNode.meta) {
    codeNode.meta = codeNode.meta.replace(/title="([^"]+)"/, 'filename="$1"');
  }
}

function isYamlCodeBlock(codeNode: CodeNode) {
  return codeNode.lang === 'yaml' || codeNode.lang === 'yml';
}

function shouldTransform(codeNode: CodeNode, mode: 'all' | 'project') {
  if (mode === 'all') {
    return true;
  }

  const filename = extractFilenameFromMeta(codeNode.meta);
  if (filename) {
    const normalizedFilename = filename.toLowerCase().replace(/\\/g, '/');
    if (isExcludedFilename(normalizedFilename)) {
      return false;
    }

    if (isLithosConfigFilename(normalizedFilename)) {
      return true;
    }
  }

  try {
    const parsed = parseYaml(codeNode.value);
    if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') {
      return false;
    }

    return Object.keys(parsed as Record<string, unknown>).some((key) =>
      PROJECT_CONFIG_KEYS.has(key)
    );
  } catch {
    return false;
  }
}

function isExcludedFilename(filename: string) {
  return (
    filename === '.lithos-state.yml' ||
    filename === '.lithos-state.yaml' ||
    filename.endsWith('/.lithos-state.yml') ||
    filename.endsWith('/.lithos-state.yaml') ||
    filename.startsWith('.github/') ||
    filename.includes('/.github/')
  );
}

function isLithosConfigFilename(filename: string) {
  return /(^|\/)lithos\.ya?ml$/.test(filename);
}

// ---------------------------------------------------------------------------
// YAML parsing with per-line path tracking. Walks the YAML AST and records
// the deepest key / value path that begins on each input line. The result
// powers `{N}` line-highlight translation across formats.
// ---------------------------------------------------------------------------

function parseYamlWithLineMap(source: string): YamlInfo | undefined {
  let doc;
  try {
    doc = parseDocument(source);
  } catch {
    return undefined;
  }
  if (doc.errors && doc.errors.length > 0) {
    return undefined;
  }

  const value = doc.toJS();
  if (value === undefined || value === null) {
    return undefined;
  }

  const lineStarts = [0];
  for (let i = 0; i < source.length; i += 1) {
    if (source[i] === '\n') {
      lineStarts.push(i + 1);
    }
  }
  const offsetToLine = (offset: number) => {
    let lo = 0;
    let hi = lineStarts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >>> 1;
      if (lineStarts[mid]! <= offset) {
        lo = mid;
      } else {
        hi = mid - 1;
      }
    }
    return lo + 1;
  };

  const lineToPath = new Map<number, Path>();
  const setLine = (line: number, path: Path) => {
    const existing = lineToPath.get(line);
    if (!existing || path.length > existing.length) {
      lineToPath.set(line, path.slice());
    }
  };

  function walk(node: any, path: Path) {
    if (!node) return;
    if (isMap(node)) {
      for (const pair of node.items as any[]) {
        if (!isPair(pair)) continue;
        const key = (pair.key as any) && (pair.key as any).value;
        if (key == null) continue;
        const childPath: Path = [...path, String(key)];
        if ((pair.key as any) && (pair.key as any).range) {
          setLine(offsetToLine((pair.key as any).range[0]), childPath);
        }
        walk(pair.value, childPath);
      }
    } else if (isSeq(node)) {
      (node.items as any[]).forEach((item, idx) => {
        const childPath: Path = [...path, idx];
        if (item && item.range) {
          setLine(offsetToLine(item.range[0]), childPath);
        }
        walk(item, childPath);
      });
    } else if (isScalar(node)) {
      if ((node as any).range) {
        setLine(offsetToLine((node as any).range[0]), path);
      }
    }
  }
  walk(doc.contents, []);

  return { value, lineToPath };
}

function pathKey(path: Path) {
  return path.map((segment) => String(segment)).join('\u0000');
}

// ---------------------------------------------------------------------------
// JSON / Luau emitters with per-path line tracking.
// ---------------------------------------------------------------------------

function buildJsonWithPathLines(rootValue: unknown): BuilderOutput {
  const builder = new LineBuilder();
  emitJsonValue(builder, rootValue, [], 0);
  return builder.finish();
}

function emitJsonValue(
  builder: LineBuilder,
  value: unknown,
  path: Path,
  indent: number
) {
  if (Array.isArray(value)) {
    if (value.length === 0) {
      builder.write('[]');
      return;
    }
    builder.write('[');
    builder.newline();
    value.forEach((item, i) => {
      const childPath: Path = [...path, i];
      builder.recordPathOnNextLine(childPath);
      builder.write('  '.repeat(indent + 1));
      emitJsonValue(builder, item, childPath, indent + 1);
      if (i < value.length - 1) builder.write(',');
      builder.newline();
    });
    builder.write('  '.repeat(indent) + ']');
    return;
  }
  if (value && typeof value === 'object') {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj);
    if (keys.length === 0) {
      builder.write('{}');
      return;
    }
    builder.write('{');
    builder.newline();
    keys.forEach((key, i) => {
      const childPath: Path = [...path, key];
      builder.recordPathOnNextLine(childPath);
      builder.write('  '.repeat(indent + 1) + JSON.stringify(key) + ': ');
      emitJsonValue(builder, obj[key], childPath, indent + 1);
      if (i < keys.length - 1) builder.write(',');
      builder.newline();
    });
    builder.write('  '.repeat(indent) + '}');
    return;
  }
  builder.write(formatJsonScalar(value));
}

function formatJsonScalar(value: unknown): string {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'number' && !Number.isFinite(value)) return 'null';
  return JSON.stringify(value);
}

function buildLuauWithPathLines(rootValue: unknown): BuilderOutput {
  const builder = new LineBuilder();
  builder.write('return ');
  emitLuauValue(builder, rootValue, [], 0);
  builder.newline();
  return builder.finish();
}

function emitLuauValue(
  builder: LineBuilder,
  value: unknown,
  path: Path,
  indent: number
) {
  if (value === null || value === undefined) {
    builder.write('nil');
    return;
  }
  if (typeof value === 'boolean') {
    builder.write(value ? 'true' : 'false');
    return;
  }
  if (typeof value === 'number') {
    builder.write(Number.isFinite(value) ? String(value) : 'nil');
    return;
  }
  if (typeof value === 'string') {
    builder.write(formatLuauString(value));
    return;
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      builder.write('{}');
      return;
    }
    builder.write('{');
    builder.newline();
    value.forEach((item, i) => {
      const childPath: Path = [...path, i];
      builder.recordPathOnNextLine(childPath);
      builder.write('  '.repeat(indent + 1));
      emitLuauValue(builder, item, childPath, indent + 1);
      builder.write(',');
      builder.newline();
    });
    builder.write('  '.repeat(indent) + '}');
    return;
  }
  if (typeof value === 'object') {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj);
    if (keys.length === 0) {
      builder.write('{}');
      return;
    }
    builder.write('{');
    builder.newline();
    keys.forEach((key, i) => {
      const childPath: Path = [...path, key];
      builder.recordPathOnNextLine(childPath);
      const formattedKey = isLuauIdentifier(key)
        ? key
        : `[${JSON.stringify(key)}]`;
      builder.write('  '.repeat(indent + 1) + formattedKey + ' = ');
      emitLuauValue(builder, obj[key], childPath, indent + 1);
      builder.write(',');
      builder.newline();
    });
    builder.write('  '.repeat(indent) + '}');
    return;
  }
  builder.write('nil');
}

function formatLuauString(value: string): string {
  if (!value.includes('\n')) {
    return JSON.stringify(value);
  }

  let level = 0;
  while (value.includes(`]${'='.repeat(level)}]`)) {
    level += 1;
  }
  const padding = '='.repeat(level);
  return `[${padding}[\n${value}]${padding}]`;
}

function isLuauIdentifier(key: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(key) && !LUAU_RESERVED.has(key);
}

class LineBuilder {
  private lines: string[] = [];
  private current = '';
  private pathToLine = new Map<string, number>();

  write(s: string) {
    this.current += s;
  }
  newline() {
    this.lines.push(this.current);
    this.current = '';
  }
  recordPathOnNextLine(path: Path) {
    const key = pathKey(path);
    if (!this.pathToLine.has(key)) {
      this.pathToLine.set(key, this.lines.length + 1);
    }
  }
  finish(): BuilderOutput {
    if (this.current.length > 0) {
      this.lines.push(this.current);
      this.current = '';
    }
    return { text: this.lines.join('\n') + '\n', pathToLine: this.pathToLine };
  }
}

// ---------------------------------------------------------------------------
// Highlight translation.
// ---------------------------------------------------------------------------

function parseHighlightMeta(meta: string | undefined): Highlight | null {
  if (!meta) return null;
  const match = meta.match(/\{([^}]+)\}/);
  if (!match || match[1] === undefined) return null;
  const lines = new Set<number>();
  for (const part of match[1].split(',')) {
    const trimmed = part.trim();
    const rangeMatch = trimmed.match(/^(\d+)\s*-\s*(\d+)$/);
    if (rangeMatch && rangeMatch[1] !== undefined && rangeMatch[2] !== undefined) {
      const start = parseInt(rangeMatch[1], 10);
      const end = parseInt(rangeMatch[2], 10);
      for (let i = start; i <= end; i += 1) {
        lines.add(i);
      }
    } else if (/^\d+$/.test(trimmed)) {
      lines.add(parseInt(trimmed, 10));
    }
  }
  return { raw: match[0], lines };
}

function formatHighlight(lineSet: Set<number>): string {
  if (!lineSet || lineSet.size === 0) return '';
  const sorted = [...lineSet].sort((a, b) => a - b);
  const parts: string[] = [];
  let i = 0;
  while (i < sorted.length) {
    let j = i;
    while (j + 1 < sorted.length && sorted[j + 1]! === sorted[j]! + 1) {
      j += 1;
    }
    parts.push(i === j ? `${sorted[i]}` : `${sorted[i]}-${sorted[j]}`);
    i = j + 1;
  }
  return `{${parts.join(',')}}`;
}

function translateHighlight(
  originalHighlight: Highlight | null,
  yamlLineToPath: Map<number, Path>,
  targetPathToLine: Map<string, number>
): Set<number> {
  const result = new Set<number>();
  if (!originalHighlight) return result;
  for (const line of originalHighlight.lines) {
    const path = yamlLineToPath.get(line);
    if (!path) continue;
    const targetLine = targetPathToLine.get(pathKey(path));
    if (targetLine !== undefined) {
      result.add(targetLine);
    }
  }
  return result;
}

// ---------------------------------------------------------------------------
// Tab construction.
// ---------------------------------------------------------------------------

function createTabsNode(
  codeNode: CodeNode,
  yamlInfo: YamlInfo,
  jsonOutput: BuilderOutput,
  luauOutput: BuilderOutput
) {
  const originalHighlight = parseHighlightMeta(codeNode.meta);

  const jsonHighlight = originalHighlight
    ? formatHighlight(
        translateHighlight(
          originalHighlight,
          yamlInfo.lineToPath,
          jsonOutput.pathToLine
        )
      )
    : '';
  const luauHighlight = originalHighlight
    ? formatHighlight(
        translateHighlight(
          originalHighlight,
          yamlInfo.lineToPath,
          luauOutput.pathToLine
        )
      )
    : '';

  return {
    type: 'mdxJsxFlowElement',
    name: 'ConfigFormatTabs',
    attributes: [],
    children: [
      createTabNode(
        'YAML',
        createCodeNode('yaml', codeNode.value, codeNode.meta)
      ),
      createTabNode(
        'JSON',
        createCodeNode(
          'json',
          jsonOutput.text,
          rewriteMetaForFormat(codeNode.meta, 'lithos.json', jsonHighlight)
        )
      ),
      createTabNode(
        'Luau',
        createCodeNode(
          'lua',
          luauOutput.text,
          rewriteMetaForFormat(codeNode.meta, 'lithos.luau', luauHighlight)
        )
      ),
    ],
    data: { _mdxExplicitJsx: true },
  };
}

function createTabNode(label: string, codeNode: ReturnType<typeof createCodeNode>) {
  return {
    type: 'mdxJsxFlowElement',
    name: 'ConfigFormatTab',
    attributes: [{ type: 'mdxJsxAttribute', name: 'label', value: label }],
    children: [codeNode],
    data: { _mdxExplicitJsx: true },
  };
}

function createCodeNode(lang: string, value: string, meta?: string) {
  return { type: 'code', lang, meta, value };
}

function rewriteMetaForFormat(
  meta: string | undefined,
  replacementName: string,
  newHighlightStr: string
): string | undefined {
  if (!meta) {
    return newHighlightStr || undefined;
  }

  let result = meta;
  const filename = extractFilenameFromMeta(meta);
  if (filename) {
    const newFilename = filename.replace(
      /lithos\.(ya?ml|json|luau|lua)$/i,
      replacementName
    );
    result = result.replace(
      /(filename|title)="[^"]+"/,
      `filename="${newFilename}"`
    );
  }

  if (/\{[^}]+\}/.test(result)) {
    result = result.replace(
      /\s*\{[^}]+\}/,
      newHighlightStr ? ` ${newHighlightStr}` : ''
    );
  } else if (newHighlightStr) {
    result = `${result.trim()} ${newHighlightStr}`;
  }

  result = result.trim();
  return result.length > 0 ? result : undefined;
}

function extractFilenameFromMeta(meta?: string) {
  const match = meta?.match(/(?:filename|title)="([^"]+)"/);
  return match?.[1];
}
