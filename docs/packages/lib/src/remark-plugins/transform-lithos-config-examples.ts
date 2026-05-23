import { parse as parseYaml } from 'yaml';
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

        const parsed = safeParseYaml(codeNode.value);
        if (parsed === undefined) {
          return;
        }

        const jsonValue = JSON.stringify(parsed, null, 2);
        const luauValue = convertToLuauReturn(parsed);

        parent.children.splice(
          index,
          1,
          createTabsNode(codeNode, jsonValue, luauValue)
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

function safeParseYaml(yamlSource: string): unknown | undefined {
  try {
    return parseYaml(yamlSource);
  } catch {
    return undefined;
  }
}

function createTabsNode(codeNode: CodeNode, jsonValue: string, luauValue: string) {
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
          jsonValue,
          rewriteFilenameMeta(codeNode.meta, 'lithos.json')
        )
      ),
      createTabNode(
        'Luau',
        createCodeNode(
          'lua',
          luauValue,
          rewriteFilenameMeta(codeNode.meta, 'lithos.luau')
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

function rewriteFilenameMeta(meta: string | undefined, replacementName: string) {
  const filename = extractFilenameFromMeta(meta);
  if (!filename) {
    return undefined;
  }

  const newFilename = filename.replace(
    /lithos\.(ya?ml|json|luau|lua)$/i,
    replacementName
  );
  return `filename="${newFilename}"`;
}

function extractFilenameFromMeta(meta?: string) {
  const match = meta?.match(/(?:filename|title)="([^"]+)"/);
  return match?.[1];
}

// ---------------------------------------------------------------------------
// Luau pretty-printer. Mirrors the JavaScript port in
// `docs/site/remark-plugins/transform-lithos-config-examples.js`.
// ---------------------------------------------------------------------------

function convertToLuauReturn(value: unknown): string {
  return `return ${formatLuauValue(value, 0)}\n`;
}

function formatLuauValue(value: unknown, indent: number): string {
  if (value === null || value === undefined) {
    return 'nil';
  }
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? String(value) : 'nil';
  }
  if (typeof value === 'string') {
    return formatLuauString(value);
  }
  if (Array.isArray(value)) {
    return formatLuauArray(value, indent);
  }
  if (typeof value === 'object') {
    return formatLuauObject(value as Record<string, unknown>, indent);
  }
  return 'nil';
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

function formatLuauArray(array: unknown[], indent: number): string {
  if (array.length === 0) {
    return '{}';
  }

  const inner = '  '.repeat(indent + 1);
  const close = '  '.repeat(indent);
  const entries = array.map(
    (item) => `${inner}${formatLuauValue(item, indent + 1)}`
  );
  return `{\n${entries.join(',\n')},\n${close}}`;
}

function formatLuauObject(
  object: Record<string, unknown>,
  indent: number
): string {
  const keys = Object.keys(object);
  if (keys.length === 0) {
    return '{}';
  }

  const inner = '  '.repeat(indent + 1);
  const close = '  '.repeat(indent);
  const entries = keys.map((key) => {
    const formattedKey = isLuauIdentifier(key) ? key : `[${JSON.stringify(key)}]`;
    const formattedValue = formatLuauValue(object[key], indent + 1);
    return `${inner}${formattedKey} = ${formattedValue}`;
  });
  return `{\n${entries.join(',\n')},\n${close}}`;
}

function isLuauIdentifier(key: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(key) && !LUAU_RESERVED.has(key);
}
