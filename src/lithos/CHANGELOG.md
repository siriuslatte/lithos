# lithos

## 0.4.0-beta.2

### Minor Changes

- Luau / Lua project configs are now supported alongside YAML and JSON via a bundled Lune evaluator, with optional `on*` lifecycle hooks (starting with `onConfigLoaded`).
- Docs site now renders every `lithos.yml` example as YAML / JSON / Luau tabs and translates line-highlight metadata across all three formats.

## 0.4.0

### Minor Changes

- Per-environment concurrency control for `deploy`, `undo`, `destroy`, and `import`. State writes are now compare-and-swap; cross-environment conflicts auto-merge, same-environment conflicts abort. Adds in-document environment locks with heartbeats and stale-lock recovery, plus a new `lithos lock list|break --environment <env>` CLI for inspecting and recovering locks left behind by crashed runs.

