# systemprompt-client Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04
**Verdict:** CLEAN

---

## Summary

| Category | Count (was -> now) |
|----------|--------------------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments on `pub` items | all (was 0) |
| Module `//!` headers | all `pub mod` (was 0) |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in public signatures | 1 -> 0 |
| Public errors typed via `thiserror` | yes (`ClientError`) |
| `[package.metadata.docs.rs] all-features = true` | yes |
| `async_trait` references | 0 |

**Total scored violations:** 0

---

## Wave A5 Fixes Applied

- `error.rs`: deleted the `Other(#[from] anyhow::Error)` variant — it had no
  in-tree callers (`grep -rn ClientError::Other` returned nothing) and put
  `anyhow::Error` on the public surface, violating §3a. Added `///` to
  `ClientResult`, `ClientError` and every variant, the named fields of
  `ApiError { status, message, details }`, the `from_response()` constructor,
  and `is_retryable()`.
- `client.rs`: added `///` to `SystempromptClient` plus the `#[must_use]`
  builder pattern annotations on `with_token`, `token`, `base_url`. Documented
  every public method (`new`, `with_timeout`, `set_token`, `list_agents`,
  `get_agent_card`, `list_contexts`, `get_context`, `create_context`,
  `create_context_auto_name`, `fetch_or_create_context`, `update_context_name`,
  `delete_context`, `list_tasks`, `delete_task`, `list_artifacts`,
  `check_health`, `verify_token`, `send_message`, `list_logs`, `list_users`,
  `get_analytics`, `list_all_artifacts`).
- `lib.rs`: added `//!` crate header with feature-flag matrix and a runnable
  `no_run` example.
- `Cargo.toml`: removed the now-unused `anyhow` workspace dep; added
  `[package.metadata.docs.rs] all-features = true`.
- `http.rs`: deliberately left `pub` (private module — clippy's
  `redundant_pub_crate` rejects narrowing).

No public signatures changed (the deleted `Other` variant was not
constructed anywhere in tree).

---

## Architectural Compliance

Layer: `shared`. Per `instructions/information/boundaries.md` dependencies must
flow downward only. Crate depends on `systemprompt-models` and
`systemprompt-identifiers` (both shared/foundation), which is sanctioned.

---

## Passing Checks

| Check | Status |
|-------|--------|
| `cargo fmt -p systemprompt-client --check` | PASS |
| `cargo build -p systemprompt-client --all-features` | PASS |
| `cargo clippy -p systemprompt-client --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-client --no-deps --all-features` | PASS |
| `just check-bans-crate systemprompt-client` | PASS |
| Public-API §3a Hygiene: rustdoc on every `pub` item | PASS |
| Public-API §3a Hygiene: `//!` on every `pub mod` | PASS |
| Public-API §3a Hygiene: `thiserror` errors in public signatures | PASS |
| Public-API §3a Hygiene: no `anyhow::Error` in public signatures | PASS |
| Public-API §3a Hygiene: `[package.metadata.docs.rs] all-features` | PASS |
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## Residual

- Some methods still take `context_id: &str` / `task_id: &str` for symmetry
  with the JSON-RPC payload format and to avoid breaking the in-tree callers
  documented in `crates/shared/client/README.md`. Tightening these to typed
  IDs (`ContextId`, `TaskId`) is a separable semver change tracked outside
  this wave.

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 4 |
| Files over 300 lines | 0 |
| Largest file | `crates/shared/client/src/client.rs` (289 lines) |

---

## Verdict

**CLEAN**

Crate meets the §3a Public-API Hygiene bar for published library code:
`anyhow` is gone from the public surface, every `pub` item carries rustdoc,
the error type is `thiserror`-derived with `#[from]` for `reqwest::Error` and
`serde_json::Error`, and docs.rs renders with `--all-features`.
