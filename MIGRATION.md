# Migrating from Mantle to Lithos

Lithos is a fork of [Mantle](https://github.com/blake-mealey/mantle) that preserves the project
model and CLI surface but renames the tool, its config files, state files, and environment
variables. Existing Mantle projects continue to work without changes thanks to backward-compatible
fallbacks.

## TL;DR

| Old (Mantle)              | New (Lithos)                | Behavior                                  |
| ------------------------- | --------------------------- | ----------------------------------------- |
| `mantle` binary           | `lithos` binary             | Same subcommands, same flags              |
| `mantle.yml`              | `lithos.yml` / `lithos.json` | All are read; discovery checks `lithos.yml`, then `lithos.json`, then legacy `mantle.yml` |
| `.mantle-state.yml`       | `.lithos-state.yml`         | Both are read; `lithos`-named wins        |
| `<key>.mantle-state.yml`  | `<key>.lithos-state.yml`    | Same fallback for remote S3 keys          |
| `MANTLE_OPEN_CLOUD_API_KEY` | `LITHOS_OPEN_CLOUD_API_KEY` | Both honored; `LITHOS_*` wins             |
| `MANTLE_AWS_ACCESS_KEY_ID`  | `LITHOS_AWS_ACCESS_KEY_ID`  | Both honored; `LITHOS_*` wins             |
| `MANTLE_AWS_SECRET_ACCESS_KEY` | `LITHOS_AWS_SECRET_ACCESS_KEY` | Both honored; `LITHOS_*` wins        |
| `MANTLE_AWS_INHERIT_IAM_ROLE` | `LITHOS_AWS_INHERIT_IAM_ROLE` | Both honored                          |

## Behavior on first deploy

When Lithos loads a project that uses the legacy names it logs a `warning:` line and continues. On
the next save:

- **State files** are written under the new `.lithos-state.yml` name. The legacy
  `.mantle-state.yml` file is left in place so that you can recover or roll back. After verifying
  the new file is correct, you can delete the legacy file.
- **Remote state** keys are written to `<key>.lithos-state.yml`. The legacy object remains in S3
  until you delete it.
- **Project config** is never rewritten by Lithos; rename `mantle.yml` to `lithos.yml` or
  `lithos.json` at your convenience.

## Recommended steps

1. Update your CI to invoke `lithos` instead of `mantle`.
2. Rename `mantle.yml` → `lithos.yml` or `lithos.json`.
3. Set `LITHOS_OPEN_CLOUD_API_KEY` and `LITHOS_AWS_*` secrets alongside (or instead of) the
   legacy `MANTLE_*` ones.
4. Run `lithos deploy` once. Confirm a fresh `.lithos-state.yml` (or remote object) is produced.
5. Delete the legacy `.mantle-state.yml` after verifying the new state.

## Documentation hosting

The documentation site moved from Vercel (`mantledeploy.vercel.app`) to GitHub Pages. Every push to
`main` triggers the [`Deploy Docs`](.github/workflows/deploy-docs.yml) workflow, which performs a
static export of the Next.js site (`docs/site` → `docs/site/out`) and publishes it via
`actions/deploy-pages`. The `NEXT_PUBLIC_BASE_PATH` environment variable controls the URL prefix
for project Pages and is set automatically from `actions/configure-pages`.
