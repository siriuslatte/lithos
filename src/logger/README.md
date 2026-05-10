# `logger`

> Internal Lithos crate. Not published to crates.io.

Tiny formatted-output helper used by the [`lithos`](../lithos) CLI. Provides
indented action blocks, colored status lines (via `yansi`), and inline diff
rendering (via `difference`).

Not a general-purpose logger — for that, use `log` + `env_logger`, which
Lithos also wires up.
