# Lithos

Infrastructure-as-code for Roblox.

Lithos lets you describe a Roblox experience in YAML or JSON and deploy it
from your terminal or CI. It manages places, badges, developer products, game
passes, thumbnails, social links, notifications, and more. Re-run the same
config and Lithos figures out what to create, update, and leave alone.

It's a continuation of [Mantle](https://github.com/blake-mealey/mantle) by
Blake Mealey. The project model and CLI stay intentionally compatible, so
existing `mantle.yml`, `mantle.yaml`, and `.mantle-state.yml` files still
work. See [MIGRATION.md](MIGRATION.md) for the rename details.

```yaml
# lithos.yml
environments:
  - label: dev
    branches: [dev]
  - label: prod
    branches: [main]

target:
  experience:
    configuration:
      genre: fighting
      playableDevices: [computer, phone, tablet]
    places:
      start:
        file: game.rbxlx
    products:
      welcomePack:
        name: Welcome Pack
        description: Starter coins and a hat.
        price: 75
```

```sh
lithos deploy --environment dev
```

## What it does

- **Plans before apply.** `deploy` previews every create, update, and delete, with field-level summaries for risky changes and explicit destructive warnings.
- **Runs preflight checks.** Lithos catches common deploy failures early: missing Open Cloud keys, wrong scopes, unsupported target-access combinations, and oversized place files.
- **Reconciles live state.** If something was deleted or changed manually in the Roblox dashboard, Lithos detects the drift instead of blindly failing an update.
- **Keeps rollback checkpoints.** Every deploy records enough history for `lithos undo` to work back toward the last known good state.
- **Handles multi-place experiences and assets.** Places, thumbnails, badges, products, audio, social links, notifications, asset aliases, and more.
- **Stores state locally or remotely.** Keep state next to the project or in shared backends such as S3 or Google Cloud Storage.

## Install

Releases are published from this repository at [`siriuslatte/lithos`](https://github.com/siriuslatte/lithos/releases).

Recommended:

**Foreman / Rokit**

```toml
# foreman.toml
[tools]
lithos = { source = "siriuslatte/lithos", version = "0.3.0" }
```

**Manual**

Download the binary for your platform from the
[releases page](https://github.com/siriuslatte/lithos/releases) and put it on
your `PATH`. The binary is named `lithos`.

**From source**

```sh
git clone https://github.com/siriuslatte/lithos
cd lithos
cargo install --path src/lithos
```

You'll need Rust 1.85 or newer.

## Configured outputs

`lithos outputs` can read its destination from project config instead of
repeating the path and `--roblox-ts` flags on every invocation.

```yaml
outputs:
  writeDir: src/shared/generated
  outputName: lithosOutputs
  format: luau
  robloxTs: true
```

Then this is enough:

```sh
lithos outputs --environment dev
```

Lithos will write `src/shared/generated/lithosOutputs.luau` plus
`src/shared/generated/lithosOutputs.d.ts`. The aliases `codegen`,
`write_dir`, `output_name`, and `typescript` are also accepted. CLI flags
still override config values.

## CLI

```
lithos deploy        Apply your project's configuration to a Roblox environment
lithos diff          Show what deploy would change
lithos undo          Restore the last recorded good snapshot for an environment
lithos destroy       Tear down everything Lithos created in an environment
lithos outputs       Print resource IDs (place IDs, asset IDs, …) or generate game-ready Luau modules
lithos import        Adopt an existing experience into Lithos
lithos state         Manage local / remote state files
```

`lithos --help` and `lithos <command> --help` cover the rest.

### Deploy preview flags

- `--yes` / `-y` — skip the interactive confirmation
- `--no-preview` — skip the preview entirely (implies `--yes`)
- `--plain-preview` — render a plain summary (no colors, no box drawing)

In CI or other non-interactive contexts, Lithos prints a plain summary and
auto-approves, so existing scripts keep working.

If a deploy turns out to be bad, `lithos undo --environment <label>` uses the
last checkpoint recorded for that environment. Undo is best-effort rather than
transactional: Lithos imports live Roblox state, previews the diff back to the
checkpoint, and applies that rollback plan.

## Community & repository process

Repository docs:

- [CONTRIBUTING.md](CONTRIBUTING.md) for repo layout, commands, schema updates, and PR expectations
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for contributor behavior expectations
- [SECURITY.md](SECURITY.md) for private vulnerability reporting
- [SUPPORT.md](SUPPORT.md) for best-effort support boundaries
- [MAINTAINERS.md](MAINTAINERS.md) for the current maintainer list

## Building and testing

```sh
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
pnpm --dir docs/site build
```

Run `pnpm install` once at the repo root to enable Husky hooks. The pre-commit
hook formats staged Rust files and checks
`cargo clippy --workspace --all-targets -- -D warnings`.

If you change config types in `src/rbx_lithos`, regenerate the committed schema
snapshot and commit it with the code change:

```sh
cargo run -p gen_schema > test/specs/schema.json
```

The live integration harness lives in `src/lithos/tests/integration.rs`, while
the canonical specs and shared assets live under `test/`. Those tests hit real
Roblox endpoints, so they are opt-in and intentionally excluded from the
default workspace test path:

```sh
cargo test -p lithos --test integration -- --test-threads=1
```

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md). It is the canonical guide for
repository layout, Rust-vs-docs workflow, validation commands, schema updates,
bug report expectations, and pull request expectations.

For support questions, read [SUPPORT.md](SUPPORT.md). For security issues, do
not open a public issue; follow [SECURITY.md](SECURITY.md).

