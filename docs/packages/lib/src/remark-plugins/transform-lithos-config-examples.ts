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

        const jsonValue = convertYamlToJson(codeNode.value);
        if (!jsonValue) {
          return;
        }

        parent.children.splice(index, 1, createTabsNode(codeNode, jsonValue));
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

function convertYamlToJson(yamlSource: string) {
  try {
    const parsed = parseYaml(yamlSource);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return undefined;
  }
}

function createTabsNode(codeNode: CodeNode, jsonValue: string) {
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

function createTabNode(label: string, codeNode: CodeNode) {
  return {
    type: 'mdxJsxFlowElement',
    name: 'ConfigFormatTab',
    attributes: [{ type: 'mdxJsxAttribute', name: 'label', value: label }],
    children: [codeNode],
    data: { _mdxExplicitJsx: true },
  };
}

function createCodeNode(codeNode: CodeNode, value: string, meta?: string) {
  return {
    type: 'code',
    lang: value === codeNode.value ? 'yaml' : 'json',
    meta,
    value,
  };
}

function buildJsonMeta(meta?: string) {
  const filename = extractFilenameFromMeta(meta);
  if (!filename) {
    return undefined;
  }

  const jsonFilename = filename.replace(/lithos\.ya?ml$/i, 'lithos.json');
  return `filename="${jsonFilename}"`;
}

function extractFilenameFromMeta(meta?: string) {
  const match = meta?.match(/(?:filename|title)="([^"]+)"/);
  return match?.[1];
}