# Audit: systemprompt-cloud (`crates/infra/cloud/`)

Date: 2026-05-15. Workspace standards audit.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Deps only on `shared/*` (identifiers, models, client) + `infra/{config,logging}`; no upward/cross-layer edges. |
| 2 | Error model | clean | `thiserror` `CloudError`/`CredentialsBootstrapError`; no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`; two `#[expect(...)]` lint attrs, both documented and legitimate. |
| 4 | Raw SQL | clean | No `sqlx::query(_)` — crate is an HTTP/credentials client with no DB access. |
| 5 | File size | clean | Largest source file 250 lines; all under the 300-line limit. |
| 6 | Function size | remediated | `run_oauth_flow` was 120 lines; extracted the axum callback closure into a `CallbackState` struct + `callback_handler` fn (now ~57 lines). |
| 7 | Async traits | clean | No trait `async fn`; `async-trait` dependency unused in trait defs. |
| 8 | Typed identifiers | clean | Service args use `TenantId`/`TransactionId`/`CheckoutSessionId`/`CloudAuthToken`. `StoredTenant.id`/`ResolvedTenant.id`/`NewCloudTenantParams.id` remain `String` — serde persistence structs with `validator` `#[validate(length)]`; converting is a cross-crate change outside this isolated worktree's safe scope. Noted, not remediated. |
| 9 | Comment standard | clean | Substantive `//!` heads on `lib.rs` and module files; zero `///` paraphrase comments; no WHAT/narration inline comments. |
| 10 | No legacy | clean | No backwards-compat shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `CloudApiClient`, `TenantStore`, `SessionStore`, `CredentialsBootstrap`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No significant repeated logic blocks. |
| 14 | CHANGELOG accuracy | clean | Entries (`report_activity`, `post_no_response`, `cancel_subscription`, `init_empty`, `update_from_tenant_info`, HTTP timeouts) all verified present in code. Note: crate `version` is 0.10.1 but CHANGELOG tops at 0.9.2 — a workspace-wide versioning lag, out of scope here. |

## Remediation summary

- Item 6: refactored `src/oauth/client.rs` — replaced the inline boxed-closure callback handler with a `CallbackState` struct and a standalone `callback_handler` axum `State` handler, matching the existing pattern in `checkout/client/handler.rs`. No behavioural change.

Verification: `cargo clippy -p systemprompt-cloud --all-targets --all-features -- -D warnings` and `cargo doc -p systemprompt-cloud --no-deps` both clean.
