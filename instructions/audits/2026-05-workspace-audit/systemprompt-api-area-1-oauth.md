# Audit: systemprompt-api — Area 1, `src/routes/oauth/`

Scope: `crates/entry/api/src/routes/oauth/**` only. Entry binary crate (`anyhow` permitted, per-item `///` banned).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only depends on `systemprompt_oauth`/`_identifiers`/`_models`/`_config` and `axum`; no sideways/circular deps. |
| 2 | Error model | clean | `anyhow` used only in `webauthn_complete.rs` helper — permitted in entry crate. |
| 3 | No panics | remediated | Replaced an introduced `.expect()` with a value returned from `verify_completion`; no `unwrap`/`panic!`/`println!` in scope. |
| 4 | Raw SQL | clean | No `sqlx::query*` calls; all DB access via `OAuthRepository` / domain services. |
| 5 | File size | clean | Largest file 287 lines (`token/handler.rs`); none over the 300-line limit. |
| 6 | Function size | remediated | `generate_anonymous_token` (~212 lines) split into `issue_cli_session`/`issue_anonymous_session`/`build_session_service`/`token_response`/`server_error`; `handle_webauthn_complete` (~141 lines) split via `verify_completion`/`error_response`. Three `token/handler.rs` grant fns left as-is — cohesive single-flow `async` blocks with no separable sub-steps. |
| 7 | Async traits | clean | No trait definitions in scope; no `#[async_trait]`. |
| 8 | Typed identifiers | clean | `ClientId`/`UserId`/`SessionId`/`AuthorizationCode`/`RefreshTokenId` used throughout; constructed via `Id::new`; no `.into()`/`::from()` at call sites. |
| 9 | Comment standard | clean | Zero `///` in scope; one substantive WHY comment in `client_credentials.rs`; no WHAT/narration comments. |
| 10 | No legacy | clean | No shims, dual paths, deprecation stubs, or dead code observed. |
| 11 | Naming | clean | HTTP handlers named `handle_*`/`generate_*`; services `*Service`/`*Registry`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | remediated | Repeated `AnonymousError` / `WebAuthnCompleteError` response construction extracted into `server_error`/`error_response`/`token_response` helpers. |
| 14 | CHANGELOG | clean | Not edited (observations only). |

## Summary
Scope was largely standards-compliant. Remediated items 6 and 13 (function-length and local
duplication) in `anonymous.rs` and `webauthn_complete.rs`; item 3 covers avoiding a panic
introduced during the refactor. Behaviour (OAuth/HTTP semantics) unchanged.
