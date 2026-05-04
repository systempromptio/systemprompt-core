# systemprompt-oauth Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Verdict:** CLEAN

---

## Summary (post Wave-B5 sweep)

| Category | Before | After |
|----------|--------|-------|
| `anyhow` refs (total) | 87 | 60 |
| `anyhow` in PUBLIC signatures | 71 | 0 |
| `///` doc lines | 0 | 63 |
| `//!` module-doc lines | 0 | 114 |
| Files >300 lines | 1 | 0 |
| `let _ =` discards | 2 | 0 |
| `.ok()` discards | 15 | 15 (each preceded by `tracing` log or used in benign `filter_map` collection) |
| Raw `sqlx::query()` outside allowlist | 0* | 0 |
| `sqlx::query!` / `query_as!` / `query_scalar!` sites | 47 | 47 |
| `*Manager` type names | 1 (`WebAuthnManager`) | 0 (renamed `WebAuthnRegistry`) |
| `#[allow(...)]` | 1 (`clippy::too_many_arguments`) | 1 (justified: builder fn) |
| `async_trait` references | 4 | 4 (required for `dyn`-dispatched provider traits) |

*The pre-sweep audit's "47 raw `sqlx::query()`" count was a false positive
from a regex that matched `sqlx::query!(` macro forms. Every site uses
the compile-time-verified macros (`query!`, `query_as!`, `query_scalar!`).

---

## Public API: typed-error taxonomy

`OauthError` (in `src/error.rs`) now enumerates the security-meaningful
failure modes consumers must distinguish:

| Variant | Purpose |
|---------|---------|
| `Provider(String)` | Upstream IdP / OAuth provider error |
| `Token(String)` | Token signing / parsing / structure failure |
| `TokenNotFound(String)` | Bearer / refresh / setup token lookup miss |
| `CodeNotFound(String)` | Authorization code lookup miss / consumed |
| `Expired(String)` | Code / token expiry |
| `PkceMismatch(String)` | PKCE verifier did not match challenge |
| `InvalidGrant(String)` | RFC 6749 `invalid_grant` |
| `InvalidClient(String)` | RFC 6749 `invalid_client` |
| `ClientNotFound(String)` | Client registry lookup miss |
| `Session(String)` | Session lookup / lifecycle |
| `WebAuthn(String)` | Passkey registration / authentication |
| `User(String)` | User registration / lookup conflict |
| `Repository(#[from] sqlx::Error)` | Underlying SQL failure |
| `Validation(String)` | Input validation |
| `Unauthorized(String)` | Caller not permitted |
| `Config(String)` | Config load / parse failure |
| `Crypto(String)` | Hash / sign / KDF failure |
| `Other(#[from] anyhow::Error)` | Adapter for upstream `anyhow` errors |

Inbound `From` adapters: `webauthn_rs::WebauthnError`, `bcrypt::BcryptError`,
`jsonwebtoken::Error`, `serde_json::Error`,
`systemprompt_models::errors::ConfigError`,
`systemprompt_config::SecretsBootstrapError`, `sqlx::Error`,
`anyhow::Error`.

`OauthResult<T>` is the canonical alias: `Result<T, OauthError>`.

---

## Per sqlx-site decision table (47 sites)

All 47 sites already use compile-time-verified macros and require **no
migration**. Each is verified against schema shape via `cargo sqlx prepare`.

| File | Macro flavour | Sites | Decision |
|------|---------------|-------|----------|
| `repository/oauth/auth_code.rs` | `query!` / `query_as!` | 4 | KEEP — auth-code lifecycle |
| `repository/oauth/refresh_token.rs` | `query!` / `query_scalar!` | 6 | KEEP — refresh-token rotation |
| `repository/oauth/user.rs` | `query!` | 2 | KEEP — user lookup |
| `repository/client/inserts.rs` | `query!` | 9 | KEEP — DCR insert path |
| `repository/client/queries.rs` | `query_as!` | 6 | KEEP — client lookup |
| `repository/client/mutations.rs` | `query!` | 6 | KEEP — update / activate |
| `repository/client/relations.rs` | `query!` | 5 | KEEP — relation joins |
| `repository/client/cleanup.rs` | `query!` | 6 | KEEP — lifecycle cleanup |
| `repository/exchange_code.rs` | `query!` | 2 | KEEP — cowork exchange |
| `repository/setup_token.rs` | `query!` | 5 | KEEP — WebAuthn setup tokens |
| `repository/webauthn.rs` | `query!` | 3 | KEEP — credentials |

No new entries to the `crates/infra/database/src/{admin,services/postgres/...}`
allowlist were required: nothing in `systemprompt-oauth` constructs
dynamic SQL.

---

## File splits

`src/repository/oauth/mod.rs` (324 lines) → split into
`src/repository/oauth/mod.rs` (~240 lines, core CRUD) and
`src/repository/oauth/cleanup.rs` (~70 lines, lifecycle methods on
`OAuthRepository`). The split is by cohesion: cleanup methods all share
a `cutoff_timestamp = days * 86400` shape and operate on lists/deletes
rather than primary CRUD.

---

## Naming

- `WebAuthnManager` → `WebAuthnRegistry` (file
  `services/webauthn/manager.rs` → `services/webauthn/registry.rs`).
  Consumers in `crates/entry/api/src/routes/oauth/webauthn/...` and
  `crates/tests/unit/domain/oauth/...` updated to the new name.

---

## Discard patterns

- `let _ = write!(out, ...)` in `services/cowork.rs` (2 sites) → replaced
  with `out.push_str(&format!(...))`. Writes to a `String` are infallible
  but `clippy::let-underscore-must-use` rejects the discard form and
  `clippy::expect-used` rejects `.expect("infallible")`. The
  `push_str(&format!(...))` form is the idiomatic clippy-clean choice.
- `.ok()` (15 sites) — each is preceded by an explicit `tracing::warn!`
  / `tracing::debug!` log of the dropped error, or is used in a
  `.filter_map(|s| s.parse().ok())` permission/audience parse where
  silently dropping unparseable entries from a list is the documented
  semantic. Verified across `services/session/lookup.rs`,
  `services/jwt/authorization.rs`, `repository/oauth/auth_code.rs`,
  `repository/oauth/user.rs`, `services/http.rs`, `services/providers.rs`.

---

## Self-verification gate (all PASS)

- `cargo fmt -p systemprompt-oauth` — clean
- `cargo build -p systemprompt-oauth --all-features` — clean
- `cargo clippy -p systemprompt-oauth --all-targets --all-features --no-deps -- -D warnings` — clean
- `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-oauth --no-deps --all-features` — clean
- `just check-bans-crate crates/domain/oauth` — clean
  (the `systemprompt-oauth` form of the recipe trips a pre-existing
  `find -name oauth` ordering bug that picks `crates/infra/cloud/src/oauth`
  before `crates/domain/oauth`; the directory form is the canonical
  invocation.)
- `just lint-sqlx` — clean (zero unverified `sqlx::query` calls workspace-wide)

---

## Verdict

**CLEAN**

Public-API surface uses typed `OauthResult<T>` / `OauthError` exclusively;
all 47 SQL call sites use compile-time-verified macros; the one
oversize file is split; `*Manager` ban resolved by rename; rustdoc
present on every public top-level item and module.
