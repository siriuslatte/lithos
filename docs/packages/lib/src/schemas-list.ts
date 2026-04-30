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
  const vscodeSnippet = `\`\`\`json
"yaml.schemas": {
  "${docsBase}/schemas/${releases[0]?.version}/schema.json": ["lithos.yml", "mantle.yml"]
}
\`\`\``;
  return (await compileMdx(vscodeSnippet)).result;
}
