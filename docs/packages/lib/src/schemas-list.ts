import { getReleases } from './releases';
import { CompileMdx } from './types';

export interface SchemaVersionItem {
  version: string;
  url: string;
}

export async function getSchemaVersionsList(): Promise<SchemaVersionItem[]> {
  const releases = await getReleases();
  return releases.map((release) => ({
    version: release.version,
    url: `/schemas/${release.version}/schema.json`,
  }));
}

export async function getSchemasSnippetContent(compileMdx: CompileMdx) {
  const releases = await getReleases();
  const docsBase =
    process.env.NEXT_PUBLIC_DOCS_BASE_URL ||
    'https://siriuslatte.github.io/lithos';
  const schemaUrl = `${docsBase}/schemas/${releases[0]?.version}/schema.json`;
  const vscodeSnippet = `### VS Code YAML files

\`\`\`json
"yaml.schemas": {
  "${schemaUrl}": ["lithos.yml", "mantle.yml"]
}
\`\`\`

### VS Code JSON files

\`\`\`json
"json.schemas": [
  {
    "fileMatch": ["lithos.json"],
    "url": "${schemaUrl}"
  }
]
\`\`\``;
  return (await compileMdx(vscodeSnippet)).result;
}
