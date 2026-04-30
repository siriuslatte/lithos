# `gen_schema`

> Internal Lithos crate. Not published to crates.io.

Generates [`specs/schema.json`](../specs/schema.json) — the JSON Schema that
powers editor autocomplete and validation for `lithos.yml` / `mantle.yml`.

The schema is derived directly from the `serde` / `schemars` annotations on
the config types in [`rbx_lithos`](../rbx_lithos), so it stays in sync with
the source of truth automatically.

## Regenerating

```sh
cargo run -p gen_schema > specs/schema.json
```

Run this whenever you change a config type and commit the result.
