const { parse: parseYaml } = require('yaml');
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

        const parsed = safeParseYaml(node.value);
        if (parsed === undefined) {
          return;
        }

        const jsonValue = JSON.stringify(parsed, null, 2);
        const luauValue = convertToLuauReturn(parsed);

        parent.children.splice(
          index,
          1,
          createTabsNode(node, jsonValue, luauValue)
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

function safeParseYaml(yamlSource) {
  try {
    return parseYaml(yamlSource);
  } catch {
    return undefined;
  }
}

function createTabsNode(codeNode, jsonValue, luauValue) {
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

function rewriteFilenameMeta(meta, replacementName) {
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

function extractFilenameFromMeta(meta) {
  const match = meta?.match(/(?:filename|title)="([^"]+)"/);
  return match?.[1];
}

// ---------------------------------------------------------------------------
// Luau pretty-printer.
//
// Converts the JSON-equivalent value parsed from a Lithos YAML config into
// a Luau `return <table>` source snippet. We aim for idiomatic output:
// bare identifiers for keys that already look like identifiers, double-quoted
// strings, long brackets for multi-line strings, and two-space indentation.
// ---------------------------------------------------------------------------

function convertToLuauReturn(value) {
  return `return ${formatLuauValue(value, 0)}\n`;
}

function formatLuauValue(value, indent) {
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
    return formatLuauObject(value, indent);
  }
  return 'nil';
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

function formatLuauArray(array, indent) {
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

function formatLuauObject(object, indent) {
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

function isLuauIdentifier(key) {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(key) && !LUAU_RESERVED.has(key);
}

module.exports = {
  createTransformLithosConfigExamples,
};