# `integration_executor`

> Internal Lithos crate. Not published to crates.io.

Test harness for end-to-end integration scenarios. Drives `rbx_lithos`
against generated YAML and synthesized image / audio assets so the deploy
path can be exercised without hand-curating fixtures.

Run via `cargo test -p integration_executor`. Tests that hit Roblox require
a `ROBLOSECURITY` (or Open Cloud key) in the environment and are gated
accordingly.
