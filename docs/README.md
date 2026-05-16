# Lithos Docs

This workspace builds the public Lithos docs site and the pull request previews
served from GitHub Pages.

## What's here

- `site/` contains the Nextra and Next.js site.
- `packages/lib/` contains shared docs code used by the site build.
- `scripts/` contains the release download and schema generation helpers used by the published docs pipeline.

## Preview flow

Pull requests targeting `dev` or `main` always get a stable `docs-preview`
check. That workflow inspects the full PR diff against the base branch.

If the diff touches docs-related paths, the preview build runs.

For pull requests opened from branches in this repository, that same workflow
deploys the preview to `gh-pages` and updates the PR comment directly.

For pull requests that do not come from this repository, a companion publish
workflow deploys the uploaded artifact to `gh-pages` and updates the PR comment
after the build succeeds.

If the diff does not touch docs-related paths, the check still succeeds and
reports that no preview was needed.

Docs-related paths include:

- `docs/**`
- `README.md`, `MIGRATION.md`, `CONTRIBUTING.md`, `SUPPORT.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `MAINTAINERS.md`
- `.github/workflows/deploy-docs.yml`
- `.github/workflows/docs-preview.yml`
- `.github/workflows/docs-preview-publish.yml`
- `.github/actions/build-docs-site/**`

Production docs publishing stays separate and only deploys the canonical site
from pushes to `main`.

## Accessing a preview

The preview URL shape is:

```text
https://siriuslatte.github.io/lithos/previews/pr-<number>
```

You can access a preview in either of these ways:

1. Open the preview comment that the publish workflow keeps updated on the pull request.
2. Replace `<number>` with the pull request number and open the URL directly.

When a pull request closes, or when its full diff no longer touches docs-related
paths, the preview is removed.

## Local docs preview

For a local docs-only preview, run `pnpm --dir docs/site dev` from the
repository root and open `http://localhost:3001/`.

For build, test, and validation commands, use [../CONTRIBUTING.md](../CONTRIBUTING.md).