# lithos

## 0.4.0

### Minor Changes

- Per-environment concurrency control for `deploy`, `undo`, `destroy`, and `import`. State writes are now compare-and-swap; cross-environment conflicts auto-merge, same-environment conflicts abort. Adds in-document environment locks with heartbeats and stale-lock recovery, plus a new `lithos lock list|break --environment <env>` CLI for inspecting and recovering locks left behind by crashed runs.

## 0.11.5

### Patch Changes

- automatically grant new audio assets permission to the target experience
