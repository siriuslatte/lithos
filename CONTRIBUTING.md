# Contributing to Lithos

Thanks for helping improve Lithos. This repository has two distinct working areas:

- the Rust workspace under `src/`
- the docs workspace under `docs/`

The goal of this guide is to keep changes easy to review and to keep local validation aligned with what maintainers actually use.

## Repository layout

Top-level directories that matter for contributors:

| Path | Purpose |
| ---- | ------- |
| `src/` | All Cargo workspace members and production Rust source |
| `test/specs/` | Repository-level YAML integration specs and the committed `schema.json` snapshot |
| `test/project-fixtures/` | Shared integration projects and fixture assets |
| `docs/` | Docs site, shared docs packages, and docs build scripts |
| `examples/` | Runnable sample projects |

The Rust workspace members under `src/` are:

| Crate path | Purpose |
| ---------- | ------- |
| `src/lithos` | CLI binary, command wiring, preview rendering, and terminal UI |
| `src/rbx_lithos` | Project loading, desired/live graph building, reconciliation, and state IO |
| `src/rbx_api` | Typed Roblox web and Open Cloud API client |
| `src/rbx_auth` | Cookie and Open Cloud credential resolution |
| `src/rbx_cookie` | ROBLOSECURITY lookup from supported local stores |
| `src/logger` | Shared bracket-prefix logger used by command output |
| `src/integration_executor` | End-to-end spec executor used by the repository integration harness |
| `src/gen_schema` | Generates the JSON Schema snapshot committed in `test/specs/schema.json` |

## Choosing the right place for a change

### Rust crates

- Put product code in the crate that owns the behavior instead of layering fixes into the CLI when the core library is the real source of truth.
- Keep crate-local unit tests next to the code they exercise.
- Keep the runnable repository integration harness in `src/lithos/tests/integration.rs`, but treat `test/specs/` and `test/project-fixtures/` as the canonical home for repository-level specs and shared assets.

If you are adding a new Roblox resource type, the usual order is:

1. Add the input/output structs in `src/rbx_lithos/src/roblox_resource_manager/`.
2. Wire the type into the desired graph builder in `src/rbx_lithos`.
3. Implement the create/update/delete behavior against `src/rbx_api`.
4. Teach the preview summarizer how to describe the field changes clearly.
5. Add a lifecycle spec under `test/specs/` that covers create, update, and destroy.

### Docs content

- Update `README.md` when repository-level workflow, structure, or install guidance changes.
- Update `docs/site/` when end-user docs need to change.
- Keep the root process docs as the canonical source for contribution, conduct, security, support, and maintainership. The docs site should link to those files rather than duplicating policy text.

### Examples and fixtures

- Keep `examples/` runnable and representative of real user workflows.
- Keep test-only assets in `test/project-fixtures/`, not in `examples/`.

## Local setup

### Rust and repo tooling

1. Install Rust 1.85 or newer.
2. Run `pnpm install` once at the repository root if you want the Husky git hooks enabled locally.
3. If you will edit docs packages or the docs site, install the docs workspace dependencies with `pnpm --dir docs install`.

### Docs workspace notes

- `pnpm --dir docs/site build` is the narrowest docs-site build check and compiles the shared docs lib first.
- `pnpm --dir docs build` runs the wider docs workspace.
- The deployment and preview workflows use the wider docs pipeline because the published site also needs the release-download and schema-build steps, not only the Next.js export.

## Pull request checks

Pull requests to `dev` or `main` expose stable checks that can be required independently:

- `rust-fmt` runs `cargo fmt --all -- --check`.
- `rust-clippy` runs `cargo clippy --workspace --all-targets -- -D warnings`.
- `workspace-gate` keeps the broader workspace build, test, schema, and CLI smoke path.
- `crate-tests / <crate>` keeps the per-crate test fan-out.
- `docs-preview` always runs on pull requests, inspects the full PR diff against the base branch, and either reports a no-op result or publishes a preview.

The docs preview URL shape is:

```text
https://siriuslatte.github.io/lithos/previews/pr-<number>
```

`docs-preview` treats these paths as docs-related:

- `docs/**`
- `README.md`, `MIGRATION.md`, `CONTRIBUTING.md`, `SUPPORT.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `MAINTAINERS.md`
- `.github/workflows/deploy-docs.yml`
- `.github/workflows/docs-preview.yml`
- `.github/workflows/docs-preview-publish.yml`
- `.github/actions/build-docs-site/**`

If the current full PR diff does not touch any of those paths, `docs-preview`
still finishes successfully so branch protection sees a stable check, and any
stale preview for that PR is removed.

The preview build itself runs in the pull request workflow. A companion publish
workflow deploys the uploaded artifact to the `gh-pages` branch and updates the
PR comment, which keeps preview deployment off the PR runner's token.

## Expected validation commands

Run the narrowest commands that match your change, but these are the baseline checks maintainers expect for ordinary Rust changes:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace --lib --bins
```

For docs-only changes, run:

```sh
pnpm --dir docs/site build
```

If you touch shared docs packages or scripts under `docs/packages/` or `docs/scripts/`, prefer:

```sh
pnpm --dir docs build
```

## Generated schema workflow

`test/specs/schema.json` is a committed generated artifact. If you change config types, schema annotations, or field docs in `src/rbx_lithos`, regenerate the snapshot and commit it with the code change:

```sh
cargo run -p gen_schema > test/specs/schema.json
```

Do not update the generated schema in a separate follow-up if the code change already depends on it.

## Live Roblox integration coverage

The default workspace test command intentionally does not run the live integration specs. Those specs:

- are discovered from `test/specs/*.yml`
- are executed through `src/lithos/tests/integration.rs`
- hit real Roblox endpoints
- require real credentials and sometimes Open Cloud scopes depending on the spec

When you explicitly want that coverage, run:

```sh
cargo test -p lithos --test integration -- --test-threads=1
```

Do not invent secrets or fake a live pass in a pull request. If you could not run the live suite locally, say so plainly in the PR.

## Filing a bug report

Start with [SUPPORT.md](SUPPORT.md) if you are not sure whether you have a bug, a usage question, or a feature request.

When you do file a bug, include:

- the smallest redacted `lithos.yml`, `lithos.yaml`, or `lithos.json` that still reproduces the problem
- the exact command you ran
- expected behavior versus actual behavior
- the relevant log output or stack trace
- your OS and `lithos --version`
- `rustc --version` if you built from source
- whether the repro uses only local validation or real Roblox endpoints
- any remote-state provider, auth mode, or environment-specific detail that matters

If the report is security-sensitive, stop and follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## Opening a pull request

Keep pull requests focused and make it easy to review the behavioral intent. A good PR description for this repository usually includes:

- what changed and why
- which crates or docs areas were touched
- whether `test/specs/schema.json` changed and why
- which validation commands you actually ran
- whether the live integration suite was run, skipped, or not applicable

Also update docs, examples, or specs in the same PR when the user-facing workflow changed.

## Support, conduct, and maintainers

- [SUPPORT.md](SUPPORT.md) explains what help is offered on a best-effort basis.
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) covers behavior expectations for issues, PRs, reviews, and docs work.
- [MAINTAINERS.md](MAINTAINERS.md) lists the current maintainer reality for this repository.