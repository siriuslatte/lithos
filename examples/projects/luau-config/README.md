# Luau Config Example

A small project that mirrors `getting-started` but defines the Lithos
config in Luau, demonstrating loops, helpers, and hook callbacks.

```sh
# From the repository root
lithos deploy --environment dev examples/projects/luau-config
```

Requires the `lune` binary on PATH. See the
[Configuration docs](../../../docs/site/pages/docs/configuration.mdx) for
installation pointers and the full hook contract.

The `game.rbxlx` referenced by `lithos.luau` is intentionally not
checked in here; copy
[`../getting-started/game.rbxlx`](../getting-started/game.rbxlx) into
this directory (or point the `file` field at your own place file)
before running `lithos deploy`.
