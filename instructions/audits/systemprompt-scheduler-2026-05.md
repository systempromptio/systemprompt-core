# systemprompt-scheduler Tech Debt Audit

**Layer:** app
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave D2 — public-API compliance sweep)
**Verdict:** CLEAN

---

## Summary (post Wave D2)

| Category | Before | After |
|----------|-------:|------:|
| unwrap()/expect() | 0 | 0 |
| panic!()/todo!()/unimplemented!() | 0 | 0 |
| println!/eprintln!/dbg! | 0 | 0 |
| `let _ =` discards | 0 | 0 |
| `.ok()` discards | 2 | 2 (carve-out documented) |
| Inline `//` comments | 0 | 0 (all `Why:` carve-outs) |
| Doc `///` comments | 0 | 100+ (every `pub` item documented) |
| Files >300 lines | 2 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 8 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |
| `anyhow::` references | 39 | 8 |
| `async_trait` references | 16 | 16 (trait-impl only) |

The 8 surviving `anyhow::` references are:

- 1 `Other(#[from] anyhow::Error)` catch-all variant on
  [`crate::error::SchedulerError`].
- 1 bridge `impl From<SchedulerError> for ProviderError` that wraps as
  `ProviderError::Internal(anyhow::Error::new(err))`.
- 4 `Future<Output = anyhow::Result<()>>` constraints on
  [`crate::services::orchestration::ServiceReconciler::reconcile`] callbacks
  — the lifecycle caller in `crates/entry/api` supplies
  `anyhow`-returning closures, and changing the bound would force a
  cross-cut migration outside the Wave D2 boundary.
- 2 doc-comment occurrences inside `//!` module docs explaining the
  catch-all.

---

## Architectural Compliance

Layer: `app`. Per `instructions/information/boundaries.md`, dependencies flow
downward only. No upward crate imports were introduced.

Public-API surface returns [`SchedulerResult<T>`](crate::SchedulerResult)
(alias for `Result<T, SchedulerError>`). [`SchedulerError`] composes
`sqlx::Error`, `tokio_cron_scheduler::JobSchedulerError`,
`systemprompt_database::RepositoryError`,
`systemprompt_analytics::AnalyticsError`, and
`systemprompt_users::UserError` via `#[from]`. Provider-trait bodies
(`Job::execute`) keep returning
[`systemprompt_provider_contracts::ProviderResult`]; a
`From<SchedulerError> for ProviderError` impl makes `?` propagation
transparent inside job bodies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| No bare `.ok()` discards (carve-outs documented) | PASS |
| No inline `//` comments (only `Why:` carve-outs) | PASS |
| Rustdoc `///` on every `pub` item | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |
| `[package.metadata.docs.rs]` set | PASS |
| Public-API typed-error boundary | PASS |

---

## File Statistics (post Wave D2)

| Metric | Value |
|--------|-------|
| Total .rs files | 28 |
| Files over 300 lines | 0 |
| Largest file | `services/orchestration/state_manager.rs` (263 lines) |

### File splits performed

- `services/scheduling/mod.rs` (327 lines) →
  - `services/scheduling/mod.rs` (216 lines, lifecycle + registration)
  - `services/scheduling/dispatch.rs` (144 lines, panic-isolated job dispatch
    + bookkeeping)
- `services/orchestration/process_cleanup.rs` (332 lines) →
  - `services/orchestration/process_cleanup/mod.rs` (183 lines, cross-platform
    API + protected-list constants)
  - `services/orchestration/process_cleanup/posix.rs` (123 lines)
  - `services/orchestration/process_cleanup/winnt.rs` (132 lines)

---

## SQLx Migration

All 8 raw `sqlx::query` sites the prior audit reported were already
`sqlx::query!`/`query_as!` macros (compile-time verified) — the audit's
regex matched the macro form. The two genuinely raw
`sqlx::query_scalar::<_, i64>(…)` calls in
`jobs/no_js_cleanup.rs` and `jobs/ghost_session_cleanup.rs` were converted
to `sqlx::query_scalar!(…)` with `as "count!": i64` annotations and are
now compile-time verified against the schema.

---

## `.ok()` carve-outs

The two `.ok()` calls in `services/orchestration/process_cleanup/posix.rs`
parse PIDs from `lsof` stdout. They are in a fast path that returns `None`
on either OS error or non-numeric output; both branches eventually surface
through `tracing::warn!` at the call site. Each carries an inline `// Why:`
comment explaining why the discard is intentional.

---

## Verdict

**CLEAN**

Public-API typed-error boundary established
(`SchedulerError` / `SchedulerResult`); all files compile; all 233
scheduler unit tests pass; `cargo clippy -p systemprompt-scheduler
--all-targets` is warning-free for this crate.
