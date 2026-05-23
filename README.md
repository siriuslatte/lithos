<p align="center">
  <img src="assets/readme-banner.svg" alt="Lithos banner" width="760" />
</p>

<p align="center">
  Infrastructure-as-code for Roblox with previewed deploys, generated outputs, and Mantle-compatible project files.
</p>

Lithos lets you describe a Roblox experience in YAML or JSON and deploy it from
your terminal or CI. It keeps the Mantle project model intentionally
compatible, so existing `mantle.yml`, `mantle.yaml`, and `.mantle-state.yml`
files still work. See [MIGRATION.md](MIGRATION.md) for the rename details.

## Quick install

Releases are published from [`siriuslatte/lithos`](https://github.com/siriuslatte/lithos/releases).

### Foreman / Rokit

```toml
# foreman.toml
[tools]
lithos = { source = "siriuslatte/lithos", version = "0.4.0-beta.2" }
```

### Manual

Download the binary for your platform from the
[releases page](https://github.com/siriuslatte/lithos/releases) and put
`lithos` on your `PATH`.

### From source

```sh
git clone https://github.com/siriuslatte/lithos
cd lithos
cargo install --path src/lithos
```

Rust 1.85 or newer is required.

## Main commands

- `lithos deploy --environment <label>` applies your config to a target environment and previews planned creates, updates, and deletes before it writes anything.
- `lithos outputs --environment <label>` prints resource IDs or generates Luau and TypeScript-friendly output modules from the `outputs` section in your config.
- `lithos diff --environment <label>` shows the deploy plan without applying it, which makes it useful for review flows and CI.
- `lithos import --environment <label>` adopts an existing Roblox experience into Lithos so the next deploy starts from managed state instead of scratch.

Run `lithos --help` or `lithos <command> --help` for the full CLI surface.

## Learn more

- [Docs site](https://siriuslatte.github.io/lithos/)
- [docs/README.md](docs/README.md) for the docs workspace and pull request preview flow
- [CONTRIBUTING.md](CONTRIBUTING.md) for building, testing, schema updates, and pull request expectations
- [MIGRATION.md](MIGRATION.md) for Mantle-to-Lithos migration notes
- [SUPPORT.md](SUPPORT.md), [SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for project process and support boundaries

