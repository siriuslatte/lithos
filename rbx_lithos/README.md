# `rbx_lithos`

> Internal Lithos crate. Not published to crates.io.

The core library that powers the [`lithos`](../lithos) CLI. Owns the resource
graph, configuration loading, state file I/O, and the reconciliation engine
that turns a `lithos.yml` into a sequence of Roblox API calls.

If you're looking for the user-facing tool, see the [repo root
README](../README.md). If you want to embed Lithos into another binary, this
is the entry point.

## Module layout

- `config` — parse and validate `lithos.yml` / `mantle.yml`.
- `project` — resolve project paths, environments, and asset overrides.
- `resource_graph` — the typed resource DAG and its `ResourceManager` trait.
- `roblox_resource_manager` — the Roblox-specific implementation of the graph
  (creates / updates / deletes via `rbx_api` and `rbxcloud`).
- `state` — versioned state file format (v1–v7), reconciliation, deployment
  history, rollback helpers, and live drift verification.
