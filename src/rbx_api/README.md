# `rbx_api`

> Internal Lithos crate. Not published to crates.io.

A typed Rust client for Roblox's web APIs. Used by [`rbx_lithos`](../rbx_lithos)
to drive deploys; not intended as a stand-alone public dependency.

## Coverage

Wraps the legacy authenticated web endpoints Lithos needs to manage:
experiences, places, developer products, game passes, badges, asset aliases
and permissions, social links, notifications, spatial voice, thumbnails, and
asset uploads (audio / image / model). Open Cloud calls live in
[`rbxcloud`](https://crates.io/crates/rbxcloud), which `rbx_lithos` uses
alongside this crate.

## Usage

```rust
use std::sync::Arc;
use rbx_auth::{RobloxCookieStore, RobloxCsrfTokenStore};
use rbx_api::RobloxApi;

let cookie_store = Arc::new(RobloxCookieStore::new()?);
let csrf_token_store = RobloxCsrfTokenStore::new();
let api = RobloxApi::new(cookie_store, csrf_token_store, None)?;

let user = api.get_authenticated_user().await?;
```

The third argument to `RobloxApi::new` is an optional Open Cloud API key; when
present, endpoints that have an Open Cloud equivalent will use it, otherwise
the cookie-authenticated path is used.

## Authentication

Authentication is handled entirely by [`rbx_auth`](../rbx_auth) — see that
crate's README for details on cookie discovery, CSRF token rotation, and the
environment variables Lithos honors.
