const {
  parse: parseYaml,
  parseDocument,
  isMap,
  isSeq,
  isScalar,
  isPair,
} = require('yaml');
const { visit } = require('unist-util-visit');

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

function createTransformLithosConfigExamples(options = {}) {
  const mode = options.mode ?? 'project';

  return function () {
    return (tree) => {
      visit(tree, 'code', (node, index, parent) => {
        if (typeof index !== 'number' || !parent) {
          return;
        }

        normalizeCodeNode(node);

        if (!isYamlCodeBlock(node) || !shouldTransform(node, mode)) {
          return;
        }

        const yamlInfo = parseYamlWithLineMap(node.value);
        if (yamlInfo === undefined) {
          return;
        }

        const jsonOutput = buildJsonWithPathLines(yamlInfo.value);
        const luauOutput = buildLuauWithPathLines(yamlInfo.value);

        parent.children.splice(
          index,
          1,
          createTabsNode(node, yamlInfo, jsonOutput, luauOutput)
        );
      });
    };
  };
}

function normalizeCodeNode(codeNode) {
  if (codeNode.lang === 'yml') {
    codeNode.lang = 'yaml';
  }

  if (codeNode.meta) {
    codeNode.meta = codeNode.meta.replace(/title="([^"]+)"/, 'filename="$1"');
  }
}

function isYamlCodeBlock(codeNode) {
  return codeNode.lang === 'yaml' || codeNode.lang === 'yml';
}

function shouldTransform(codeNode, mode) {
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

    return Object.keys(parsed).some((key) => PROJECT_CONFIG_KEYS.has(key));
  } catch {
    return false;
  }
}

function isExcludedFilename(filename) {
  return (
    filename === '.lithos-state.yml' ||
    filename === '.lithos-state.yaml' ||
    filename.endsWith('/.lithos-state.yml') ||
    filename.endsWith('/.lithos-state.yaml') ||
    filename.startsWith('.github/') ||
    filename.includes('/.github/')
  );
}

function isLithosConfigFilename(filename) {
  return /(^|\/)lithos\.ya?ml$/.test(filename);
}

// ---------------------------------------------------------------------------
// YAML parsing with per-line path tracking.
//
// We walk the YAML AST (via `parseDocument`) and record, for each input line,
// the deepest key/value path that begins on that line. This lets us translate
// a user's `{4,6-7}` line-highlight meta into the equivalent lines in the
// generated JSON / Luau tabs.
// ---------------------------------------------------------------------------

function parseYamlWithLineMap(source) {
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
  const offsetToLine = (offset) => {
    let lo = 0;
    let hi = lineStarts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >>> 1;
      if (lineStarts[mid] <= offset) {
        lo = mid;
      } else {
        hi = mid - 1;
      }
    }
    return lo + 1;
  };

  // For each line, record the deepest path that starts there. Walking
  // depth-first means deeper paths overwrite shallower ones at the same line.
  const lineToPath = new Map();
  const setLine = (line, path) => {
    const existing = lineToPath.get(line);
    if (!existing || path.length > existing.length) {
      lineToPath.set(line, path.slice());
    }
  };

  function walk(node, path) {
    if (!node) return;
    if (isMap(node)) {
      for (const pair of node.items) {
        if (!isPair(pair)) continue;
        const key = pair.key && pair.key.value;
        if (key == null) continue;
        const childPath = [...path, String(key)];
        if (pair.key && pair.key.range) {
          setLine(offsetToLine(pair.key.range[0]), childPath);
        }
        walk(pair.value, childPath);
      }
    } else if (isSeq(node)) {
      node.items.forEach((item, idx) => {
        const childPath = [...path, idx];
        if (item && item.range) {
          setLine(offsetToLine(item.range[0]), childPath);
        }
        walk(item, childPath);
      });
    } else if (isScalar(node)) {
      if (node.range) {
        setLine(offsetToLine(node.range[0]), path);
      }
    }
  }
  walk(doc.contents, []);

  return { value, lineToPath };
}

function pathKey(path) {
  return path.map((segment) => String(segment)).join('\u0000');
}

// ---------------------------------------------------------------------------
// JSON / Luau emitters with per-path line tracking.
//
// Each emitter produces { text, pathToLine }. `pathToLine` maps a `pathKey`
// (the `\0`-joined path from the root) to the 1-indexed output line where
// that key / value pair starts. The line-highlight translator looks up the
// YAML path for each highlighted YAML line, then looks up the corresponding
// line in the target format.
// ---------------------------------------------------------------------------

function buildJsonWithPathLines(rootValue) {
  const builder = new LineBuilder();
  emitJsonValue(builder, rootValue, [], 0);
  return builder.finish();
}

function emitJsonValue(builder, value, path, indent) {
  if (Array.isArray(value)) {
    if (value.length === 0) {
      builder.write('[]');
      return;
    }
    builder.write('[');
    builder.newline();
    value.forEach((item, i) => {
      const childPath = [...path, i];
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
    const keys = Object.keys(value);
    if (keys.length === 0) {
      builder.write('{}');
      return;
    }
    builder.write('{');
    builder.newline();
    keys.forEach((key, i) => {
      const childPath = [...path, key];
      builder.recordPathOnNextLine(childPath);
      builder.write('  '.repeat(indent + 1) + JSON.stringify(key) + ': ');
      emitJsonValue(builder, value[key], childPath, indent + 1);
      if (i < keys.length - 1) builder.write(',');
      builder.newline();
    });
    builder.write('  '.repeat(indent) + '}');
    return;
  }
  builder.write(formatJsonScalar(value));
}

function formatJsonScalar(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'number' && !Number.isFinite(value)) return 'null';
  return JSON.stringify(value);
}

function buildLuauWithPathLines(rootValue) {
  const builder = new LineBuilder();
  builder.write('return ');
  emitLuauValue(builder, rootValue, [], 0);
  builder.newline();
  return builder.finish();
}

function emitLuauValue(builder, value, path, indent) {
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
      const childPath = [...path, i];
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
    const keys = Object.keys(value);
    if (keys.length === 0) {
      builder.write('{}');
      return;
    }
    builder.write('{');
    builder.newline();
    keys.forEach((key, i) => {
      const childPath = [...path, key];
      builder.recordPathOnNextLine(childPath);
      const formattedKey = isLuauIdentifier(key)
        ? key
        : `[${JSON.stringify(key)}]`;
      builder.write('  '.repeat(indent + 1) + formattedKey + ' = ');
      emitLuauValue(builder, value[key], childPath, indent + 1);
      builder.write(',');
      builder.newline();
    });
    builder.write('  '.repeat(indent) + '}');
    return;
  }
  builder.write('nil');
}

function formatLuauString(value) {
  if (!value.includes('\n')) {
    return JSON.stringify(value);
  }

  // Use Lua long brackets for multi-line strings, picking an equals padding
  // that does not appear in the body so we never accidentally close early.
  let level = 0;
  while (value.includes(`]${'='.repeat(level)}]`)) {
    level += 1;
  }
  const padding = '='.repeat(level);
  return `[${padding}[\n${value}]${padding}]`;
}

function isLuauIdentifier(key) {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(key) && !LUAU_RESERVED.has(key);
}

class LineBuilder {
  constructor() {
    this.lines = [];
    this.current = '';
    this.pathToLine = new Map();
  }
  write(s) {
    this.current += s;
  }
  newline() {
    this.lines.push(this.current);
    this.current = '';
  }
  recordPathOnNextLine(path) {
    const key = pathKey(path);
    if (!this.pathToLine.has(key)) {
      this.pathToLine.set(key, this.lines.length + 1);
    }
  }
  finish() {
    if (this.current.length > 0) {
      this.lines.push(this.current);
      this.current = '';
    }
    return { text: this.lines.join('\n') + '\n', pathToLine: this.pathToLine };
  }
}

// ---------------------------------------------------------------------------
// Highlight translation. Parses the `{N,M-O}` portion of the YAML meta, maps
// each highlighted YAML line through the path table to the equivalent line
// in the JSON / Luau output, and serializes the result back into the same
// `{...}` syntax that Nextra / shiki expects.
// ---------------------------------------------------------------------------

function parseHighlightMeta(meta) {
  if (!meta) return null;
  const match = meta.match(/\{([^}]+)\}/);
  if (!match) return null;
  const lines = new Set();
  for (const part of match[1].split(',')) {
    const trimmed = part.trim();
    const rangeMatch = trimmed.match(/^(\d+)\s*-\s*(\d+)$/);
    if (rangeMatch) {
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

function formatHighlight(lineSet) {
  if (!lineSet || lineSet.size === 0) return '';
  const sorted = [...lineSet].sort((a, b) => a - b);
  const parts = [];
  let i = 0;
  while (i < sorted.length) {
    let j = i;
    while (j + 1 < sorted.length && sorted[j + 1] === sorted[j] + 1) {
      j += 1;
    }
    parts.push(i === j ? `${sorted[i]}` : `${sorted[i]}-${sorted[j]}`);
    i = j + 1;
  }
  return `{${parts.join(',')}}`;
}

function translateHighlight(originalHighlight, yamlLineToPath, targetPathToLine) {
  const result = new Set();
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

function createTabsNode(codeNode, yamlInfo, jsonOutput, luauOutput) {
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

function createTabNode(label, codeNode) {
  return {
    type: 'mdxJsxFlowElement',
    name: 'ConfigFormatTab',
    attributes: [{ type: 'mdxJsxAttribute', name: 'label', value: label }],
    children: [codeNode],
    data: { _mdxExplicitJsx: true },
  };
}

function createCodeNode(lang, value, meta) {
  return { type: 'code', lang, meta, value };
}

function rewriteMetaForFormat(meta, replacementName, newHighlightStr) {
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

function extractFilenameFromMeta(meta) {
  const match = meta?.match(/(?:filename|title)="([^"]+)"/);
  return match?.[1];
}

module.exports = {
  createTransformLithosConfigExamples,
};
