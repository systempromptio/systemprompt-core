# Audit — systemprompt-oauth

Crate: `crates/domain/oauth/` — OAuth 2.0 / OIDC, WebAuthn, JWT, CIMD. Audited 2026-05-15.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only on `shared`/`infra` crates; no upward or cross-domain deps. |
| 2 | Error model | clean | `thiserror`-derived `OauthError` in `error.rs`; no `anyhow` in deps or signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`; only `unwrap_or_else` fallbacks. |
| 4 | Raw SQL | clean | All persistence via compile-time-verified `query!`/`query_as!`/`query_scalar!`; no runtime `sqlx::query(_)`. |
| 5 | File size | clean | Largest file 265 lines (`services/generation.rs`); all under the 300-line limit. |
| 6 | Function size | clean | No functions materially over the ~75-line guidance; cohesive sub-modules already split. |
| 7 | Async traits | remediated | `TokenValidator` was `#[async_trait]` with no `dyn` use anywhere; converted to native `async fn` (RPITIT), dropped the `async-trait` dependency. |
| 8 | Typed identifiers | clean | `systemprompt_identifiers` types used across models/repositories; no raw `String` entity IDs (`rp_id` is a WebAuthn protocol string, not an entity ID). |
| 9 | Comment standard | clean | Substantive `//!` heads; the only `///` block (`services/bridge.rs`) and the one inline `//` (RFC 7591 reference) encode genuine WHY. |
| 10 | No legacy | clean | No backwards-compat shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `*Service` / `*Handler` used appropriately; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No significant repeated logic warranting extraction. |
| 14 | CHANGELOG accuracy | clean | Recent entries (0.9.2 bridge rename, 0.4.3 audience change) match the current code surface. |

## Remediation summary

- Item 7: `TokenValidator` (`services/jwt/mod.rs`) had no trait-object usage in the crate or workspace, so the `#[async_trait]` macro was unnecessary. Replaced with a native `async fn` expressed as `-> impl Future<...> + Send` (RPITIT). The single impl (`JwtTokenValidator` in `services/webauthn/jwt.rs`) was de-annotated to a plain `async fn`. The now-unused `async-trait` dependency was removed from `Cargo.toml`. No behavioural change.

## Verification

- `SQLX_OFFLINE=true cargo clippy -p systemprompt-oauth --all-targets --all-features -- -D warnings` — clean.
- `SQLX_OFFLINE=true cargo doc -p systemprompt-oauth --no-deps` — clean.
