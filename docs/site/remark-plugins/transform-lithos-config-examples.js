const { parse: parseYaml } = require('yaml');
const { visit } = require('unist-util-visit');

const PROJECT_CONFIG_KEYS = new Set([
  'owner',
  'payments',
  'environments',
  'target',
  'state',
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

        const jsonValue = convertYamlToJson(node.value);
        if (!jsonValue) {
          return;
        }

        parent.children.splice(index, 1, createTabsNode(node, jsonValue));
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

function convertYamlToJson(yamlSource) {
  try {
    const parsed = parseYaml(yamlSource);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return undefined;
  }
}

function createTabsNode(codeNode, jsonValue) {
  return {
    type: 'mdxJsxFlowElement',
    name: 'ConfigFormatTabs',
    attributes: [],
    children: [
      createTabNode('YAML', createCodeNode(codeNode, codeNode.value, codeNode.meta)),
      createTabNode(
        'JSON',
        createCodeNode(codeNode, jsonValue, buildJsonMeta(codeNode.meta))
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

function createCodeNode(codeNode, value, meta) {
  return {
    type: 'code',
    lang: value === codeNode.value ? 'yaml' : 'json',
    meta,
    value,
  };
}

function buildJsonMeta(meta) {
  const filename = extractFilenameFromMeta(meta);
  if (!filename) {
    return undefined;
  }

  const jsonFilename = filename.replace(/lithos\.ya?ml$/i, 'lithos.json');
  return `filename="${jsonFilename}"`;
}

function extractFilenameFromMeta(meta) {
  const match = meta?.match(/(?:filename|title)="([^"]+)"/);
  return match?.[1];
}

module.exports = {
  createTransformLithosConfigExamples,
};