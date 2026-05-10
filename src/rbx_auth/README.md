# `rbx_auth`

> Internal Lithos crate. Not published to crates.io.

Authentication helpers for Roblox's legacy web APIs. Resolves a
`.ROBLOSECURITY` cookie from the user's environment or local Roblox Studio
installation and manages the rotating `X-Csrf-Token` header that Roblox
requires for state-changing requests.

Used by [`rbx_api`](../rbx_api) and, transitively, by
[`rbx_lithos`](../rbx_lithos). Not intended as a stand-alone public
dependency.

## What it provides

- `RobloxCookieStore` — a `reqwest_cookie_store::CookieStoreMutex` pre-loaded
  with a `.ROBLOSECURITY` cookie discovered via [`rbx_cookie`](../rbx_cookie).
  Plug it into a `reqwest::Client` with `.cookie_provider(...)`.
- `RobloxCsrfTokenStore` — wraps a `reqwest::Client` request so that
  `X-Csrf-Token` is captured on the first 403, refreshed automatically, and
  re-sent on the retry. Survives token rotation without leaking state across
  requests.

## Cookie discovery

The cookie is resolved in this order (see [`rbx_cookie`](../rbx_cookie) for
the implementation):

1. The `ROBLOSECURITY` environment variable.
2. The authenticated local Roblox Studio installation
   (Windows Credential Manager / macOS BinaryCookies).

In CI you'll typically supply a `ROBLOSECURITY` secret. Locally, just be
logged into Studio and Lithos will pick the cookie up automatically.

## Library usage

```rust
use std::sync::Arc;
use rbx_auth::{RobloxCookieStore, RobloxCsrfTokenStore};

let cookie_store = Arc::new(RobloxCookieStore::new()?);
let csrf_token_store = RobloxCsrfTokenStore::new();

let client = reqwest::Client::builder()
    .user_agent("Roblox/WinInet")
    .cookie_provider(cookie_store)
    .build()?;

let res = csrf_token_store
    .send_request(|| async {
        Ok(client.get("https://users.roblox.com/v1/users/authenticated"))
    })
    .await?;
```

## CLI

A small CLI is included primarily for local debugging. Build it from the
workspace with `cargo build -p rbx_auth` and run `./target/debug/rbx_auth
--help`. Disable the `cli` feature for a leaner library-only build.
