# Lithos

Infrastructure-as-code for Roblox.

Lithos lets you describe a Roblox experience — places, badges, developer
products, game passes, thumbnails, social links, notifications, and the like —
in a single YAML file, and then deploy that description from your terminal or
from CI. Run it again and it figures out what to create, what to update, and
what to leave alone. No clicking through the creator dashboard, no forgetting
to flip a setting, no "wait, which build is on prod?".

It's a continuation of [Mantle](https://github.com/blake-mealey/mantle) by
Blake Mealey. The project model and CLI surface are intentionally the same;
existing `mantle.yml` and `.mantle-state.yml` files keep working. See
[MIGRATION.md](MIGRATION.md) for the rename details.

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

- **Plans before it touches anything.** `deploy` shows a preview of every create / update / delete it's about to perform, with field-level summaries for risky resources (a price change on a developer product, a name change on a game pass) and explicit warnings on destructive operations. You can approve or cancel before a single byte hits Roblox.
- **Verifies live state.** Before applying, Lithos checks the persisted state against the actual Roblox API. If something was deleted manually in the dashboard, Lithos notices and re-creates it instead of failing on an update.
- **Manages multi-place experiences.** Start places, side places, place files, configurations, thumbnails, icons — all of it.
- **Handles the assets too.** Images, audio, badges, game passes, developer products, notifications, asset aliases, social links, spatial voice — the things that usually get forgotten on launch day.
- **Stores state where you want it.** Locally next to your project, or remotely in S3 / Google Cloud Storage so a team or a CI runner can cooperate on the same experience.

## Install

Releases are published from this repository at [`siriuslatte/lithos`](https://github.com/siriuslatte/lithos/releases).

The simplest path:

**Foreman / Rokit**

```toml
# foreman.toml
[tools]
lithos = { source = "siriuslatte/lithos", version = "0.1.0" }
```

**Manual**

Download the binary for your platform from the
[releases page](https://github.com/siriuslatte/lithos/releases) and put it on
your `PATH`. The binary is named `lithos`.

**From source**

```sh
git clone https://github.com/siriuslatte/lithos
cd lithos
cargo install --path lithos
```

You'll need Rust 1.79 or newer.

## Quick start

There's a `getting-started` project under [`examples/`](examples/). Clone the
repo and try it:

```sh
cd examples
lithos deploy projects/getting-started --environment dev
```

The first run creates an experience and a place; subsequent runs only push what changed. Run `lithos diff --environment dev` any time to see what would happen without actually deploying.

If you're not signed into Roblox Studio on the same machine, set `ROBLOSECURITY` (and optionally `LITHOS_OPEN_CLOUD_API_KEY` for Open Cloud endpoints).

## CLI

```
lithos deploy        Apply your project's configuration to a Roblox environment
lithos diff          Show what deploy would change
lithos destroy       Tear down everything Lithos created in an environment
lithos outputs       Print resource IDs (place IDs, asset IDs, …) for use in your game
lithos import        Adopt an existing experience into Lithos
lithos state         Manage local / remote state files
```

`lithos --help` and `lithos <command> --help` have the rest.

### Deploy preview flags

- `--yes` / `-y` — skip the interactive confirmation
- `--no-preview` — skip the preview entirely (implies `--yes`)
- `--plain-preview` — render a plain summary (no colors, no box drawing)

In CI / piped contexts Lithos auto-approves after printing a plain summary, so
existing scripts keep working without changes.

## Repository layout

This is a Cargo workspace. The pieces:

| Crate                  | Purpose                                                              |
| ---------------------- | -------------------------------------------------------------------- |
| `lithos`               | The CLI binary. Commands, plan preview, branded UI chrome.           |
| `rbx_lithos`           | Project loading, resource graph, reconciliation, state IO.           |
| `rbx_api`              | Typed Roblox web / Open Cloud API client.                            |
| `rbx_auth`             | Cookie + Open Cloud key resolution.                                  |
| `rbx_cookie`           | Reads `.ROBLOSECURITY` from Studio's keychain / Windows credentials. |
| `gen_schema`           | Emits the JSON schema for `lithos.yml`.                              |
| `integration_executor` | Drives end-to-end YAML specs in `specs/`.                            |
| `logger`               | The bracket-prefix tree logger you see in command output.            |

`docs/` is the docs site (Next.js + Nextra). `examples/` is a handful of
runnable projects.

## Building and testing

```sh
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy -- -D warnings
cargo fmt
```

Integration tests under `specs/*.yml` hit real Roblox endpoints. They're opt-in and gated by environment variables; CI runs them in a separate workflow against a dedicated test account.

## Contributing

Bug reports and PRs welcome. Try to include a minimal `lithos.yml` that
reproduces the issue. The
[`.github/ISSUE_TEMPLATE`](.github/ISSUE_TEMPLATE) folder has the templates.

If you're adding a new resource type, the rough order is:

1. Add the inputs / outputs structs in `rbx_lithos/src/roblox_resource_manager/`.
2. Wire them into the desired-graph builder.
3. Implement the create / update / delete operations against `rbx_api`.
4. Teach the preview's `summarize` module how to describe field changes.
5. Add a spec under `specs/` covering the create → update → destroy lifecycle.

## License

MIT. See [LICENSE](LICENSE).

Mantle was originally created by
[Blake Mealey](https://github.com/blake-mealey). This continuation builds on
his work and the contributions of everyone who helped shape the original tool.
