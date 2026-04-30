# Lithos Examples

Runnable example projects for learning Lithos.

| Project                                       | What it shows                                                                                                                                |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| [`getting-started`](projects/getting-started) | The smallest valid `lithos.yml`: one experience, one place, two environments.                                                                |
| [`pirate-wars`](projects/pirate-wars)         | A near-complete project: multi-place, icon, thumbnails, products, passes, badges, social links, notifications, asset bundle, env overrides. |

## Usage

```sh
# 1. Clone the repo
git clone https://github.com/siriuslatte/lithos
cd lithos/examples

# 2. Install Lithos with Foreman (or Rokit). See foreman.toml in this folder.
foreman install

# 3. Deploy a project. All examples define `dev` and `prod` environments;
#    `dev` is private and prefixed with `[DEV]`, `prod` is public.
lithos deploy projects/getting-started --environment dev

# 4. Preview future changes without applying them.
lithos diff projects/getting-started --environment dev

# 5. Tear it down when you're done.
lithos destroy projects/getting-started --environment dev
```

If you're not signed into Roblox Studio on the same machine, set the `ROBLOSECURITY` environment variable. Set `LITHOS_OPEN_CLOUD_API_KEY` if you want to use Open Cloud endpoints (e.g. notifications).

## Tinkering

The fastest way to learn Lithos is to edit a `lithos.yml`, run `lithos diff --environment dev`, and watch what it would change. Suggested exercises against `pirate-wars`:

- Bump a developer product's `price` and see the field-level diff.
- Add a third badge.
- Toggle `targetAccess` on the dev environment between `private` and `friends`.
- Move the state file to S3 by uncommenting the `state.remote` block.

Existing Mantle projects keep working — Lithos reads `mantle.yml` and `.mantle-state.yml` as fallbacks. See the top-level
[`MIGRATION.md`](../MIGRATION.md) for details.

