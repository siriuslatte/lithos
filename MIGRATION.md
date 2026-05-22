# Migrating from Mantle to Lithos

Lithos is a fork of [Mantle](https://github.com/blake-mealey/mantle) that preserves the project
model and CLI surface but renames the tool, its config files, state files, and environment
variables. Existing Mantle projects continue to work without changes thanks to backward-compatible
fallbacks.

## TL;DR

| Old (Mantle)              | New (Lithos)                | Behavior                                  |
| ------------------------- | --------------------------- | ----------------------------------------- |
| `mantle` binary           | `lithos` binary             | Same subcommands, same flags              |
| `mantle.yml` / `mantle.yaml` | `lithos.yml` / `lithos.yaml` / `lithos.json` | All are read; discovery checks `lithos.yml`, then `lithos.yaml`, then `lithos.json`, then legacy `mantle.yml`, `mantle.yaml` |
| `.mantle-state.yml`       | `.lithos-state.yml`         | Both are read; `lithos`-named wins        |
| `<key>.mantle-state.yml`  | `<key>.lithos-state.yml`    | Same fallback for remote S3 keys          |
| `MANTLE_OPEN_CLOUD_API_KEY` | `LITHOS_OPEN_CLOUD_API_KEY` | Both honored; `LITHOS_*` wins. `ROBLOX_OPEN_CLOUD_API_KEY` is also accepted as an alias. |
| `MANTLE_AWS_ACCESS_KEY_ID`  | `LITHOS_AWS_ACCESS_KEY_ID`  | Both honored; `LITHOS_*` wins             |
| `MANTLE_AWS_SECRET_ACCESS_KEY` | `LITHOS_AWS_SECRET_ACCESS_KEY` | Both honored; `LITHOS_*` wins        |
| `MANTLE_AWS_INHERIT_IAM_ROLE` | `LITHOS_AWS_INHERIT_IAM_ROLE` | Both honored                          |

## Behavior on first deploy

When Lithos loads a project that uses the legacy names it logs a `warning:` line and continues. On
the next save:

- **State files** are written under the new `.lithos-state.yml` name. The legacy
  `.mantle-state.yml` file is left in place so that you can recover or roll back. New Lithos
  writes use the v7 format, which stores both the current environment graph and recent deployment
  checkpoints for `lithos undo`. After verifying the new file is correct, you can delete the legacy
  file.
- **Remote state** keys are written to `<key>.lithos-state.yml`. The legacy object remains in S3
  until you delete it.
- **Project config** is never rewritten by Lithos; rename `mantle.yml` / `mantle.yaml` to
  `lithos.yml` / `lithos.yaml` at your convenience, or convert it to JSON and
  save it as `lithos.json`.

## Recommended steps

1. Update your CI to invoke `lithos` instead of `mantle`.
2. Rename `mantle.yml` → `lithos.yml` or `mantle.yaml` → `lithos.yaml`. If you prefer JSON, convert the
  config and save it as `lithos.json`.
3. Set `LITHOS_OPEN_CLOUD_API_KEY` and `LITHOS_AWS_*` secrets alongside (or instead of) the
  legacy `MANTLE_*` ones. Lithos also accepts `ROBLOX_OPEN_CLOUD_API_KEY`, but `LITHOS_*` is the
  preferred name in project docs and CI.
4. Run `lithos deploy` once. Confirm a fresh `.lithos-state.yml` (or remote object) is produced.
5. Delete the legacy `.mantle-state.yml` after verifying the new state.

## Concurrency model (new in Lithos)

Lithos serializes mutating commands per environment via an in-document
lock plus compare-and-swap on every state write. This is a behavior
change from Mantle, which trusted the shared remote state file as the
only coordination point.

Practical consequences when migrating:

- The state document now has a top-level `locks:` map keyed by
  environment label. It is `#[serde(default)]`, so older state files
  continue to load unchanged; the field is added the first time Lithos
  writes after acquiring a lock.
- Two concurrent runs against the same environment no longer race. The
  second one fails immediately with a diagnostic pointing at
  `lithos lock break --environment <env>`. Cross-environment runs still
  proceed in parallel.
- Crashed or killed runs leave behind a lock that goes stale after
  15 minutes (heartbeats are written on every progress update) and is
  reclaimed automatically. Use `lithos lock list` to inspect, and
  `lithos lock break --environment <env>` to recover faster.
- For S3 / R2 backends, compare-and-swap is best-effort
  (load-then-write); the in-document lock is the authoritative protection
  against multi-writer corruption. Local state additionally uses an
  atomic tmp + rename on save.

Full details: **[State and reconciliation → Concurrency](docs/site/pages/docs/concepts/state.mdx)**.

## Documentation hosting

The documentation site moved from Vercel (`mantledeploy.vercel.app`) to GitHub Pages. Every push to
`main` triggers the [`Deploy Docs`](.github/workflows/deploy-docs.yml) workflow, which performs a
static export of the Next.js site (`docs/site` → `docs/site/out`) and publishes it via
`actions/deploy-pages`. The `NEXT_PUBLIC_BASE_PATH` environment variable controls the URL prefix
for project Pages and is set automatically from `actions/configure-pages`.