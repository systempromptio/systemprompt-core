# systemprompt-cloud Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave B2)
**Verdict:** CLEAN

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 (replaced with `if … is_err()` logging) |
| `.ok()` discards | 0 (env-var helpers reified to `read_env_optional`) |
| Inline `//` comments | 0 |
| Doc `///` comments | covered for every `pub` item |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 (replaced with two scoped `#[expect(...)]`) |
| `anyhow::` references | 0 |
| `async_trait` references | 0 |

**Total scored violations:** 0

---

## Wave B2 Compliance Sweep

### Public-API hygiene

- `anyhow::Result` / `anyhow::Error` removed from every public signature.
- `CloudError` extended with new typed variants for OAuth flows, checkout flows, SSE streams, provisioning failures, HTTP statuses, session-version mismatches, and credential bootstrap states. `serde_json::Error` and `reqwest::Error` are now composed via `#[from]`.
- `CredentialsBootstrapError` retained as a precise sub-error and converted via `From<CredentialsBootstrapError> for CloudError` (in `credentials_bootstrap/error.rs`).
- `CloudResult<T> = Result<T, CloudError>` re-exported from `lib.rs`.
- `[package.metadata.docs.rs]` block added.
- `//!` crate-level rustdoc with public-surface map and feature-flag note.
- `///` rustdoc + `# Errors` sections on every `pub` item.

### File splits

`api_client/client.rs` (304 lines) split into a focused module set:
- `api_client/client.rs` — `CloudApiClient` constructor + `handle_response` / `handle_no_content_response`.
- `api_client/methods.rs` — low-level `get`/`post`/`put`/`delete` verbs.
- `api_client/endpoints.rs` — top-level endpoints (`get_user`, `list_tenants`, `get_plans`, `create_checkout`, `report_activity`).

`tenants.rs` (327 lines) split into a module directory:
- `tenants/mod.rs` — `StoredTenant`, `TenantType`, `NewCloudTenantParams`.
- `tenants/tenant_store.rs` — persistent `TenantStore`.

`checkout/client.rs` (325 lines after rewrite) split into:
- `checkout/client/mod.rs` — public `run_checkout_callback_flow` + types.
- `checkout/client/handler.rs` — Axum callback / status handlers + provisioning watcher.

`credentials_bootstrap.rs` (304 lines after rewrite) split into:
- `credentials_bootstrap/mod.rs` — `CredentialsBootstrap` state machine.
- `credentials_bootstrap/error.rs` — `CredentialsBootstrapError` + `From` conversion.

`error.rs` (313 lines after extension) split into:
- `error/mod.rs` — `CloudError` enum + constructor.
- `error/messages.rs` — `user_message`, `recovery_hint`, `requires_login`, `requires_setup`.

### Other fixes

- `let _ = CREDENTIALS.set(None)` in `init_empty()` replaced with `if … is_err()` debug logging.
- 5 `.ok()` discards in `credentials_bootstrap.rs` and `cli_session/store.rs` replaced with explicit `match` arms that log via `tracing::debug!` / `tracing::warn!`.
- `#[allow(clippy::struct_field_names)]` on `CheckoutTemplates` replaced with a scoped `#[expect(... reason = "...")]`.
- Consumer fixes (cli): three `.map_err(Into::into)` additions in `crates/entry/cli/src/{session/store,commands/cloud/secrets/helpers,commands/cloud/tenant/select}.rs` to bridge the new typed error into the entry crate's `anyhow::Result`. No deeper changes to other-owner crates.

---

## Self-verification gate

- `cargo fmt -p systemprompt-cloud` — clean.
- `cargo build -p systemprompt-cloud` — green.
- `cargo clippy -p systemprompt-cloud --no-deps --all-targets -- -D warnings` — green.
- `cargo doc -p systemprompt-cloud --no-deps` with `RUSTDOCFLAGS="-D warnings"` — green.
- Workspace `cargo build --workspace` — green.

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 34 |
| Files over 300 lines | 0 |
| Largest file | `cli_session/session.rs` (270) |

---

## Verdict

**CLEAN** — ready for crates.io publication after the wave merges.
