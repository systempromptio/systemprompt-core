# Audit: systemprompt-sync (`crates/app/sync/`)

Date: 2026-05-16. Scope: standards + safe refactors only; no behavioural or cross-crate public-signature changes.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Deps flow downward only — app → domain (`agent`, `content`), infra (`database`, `security`, `logging`), shared (`models`, `traits`, `identifiers`, `provider-contracts`). No upward/cross-layer deps. |
| 2 | Error model | clean | `domain_error!`-generated `thiserror` enum in `error.rs`; no `anyhow` anywhere; no `anyhow::Error` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`/`todo!`. `try_from(...).unwrap_or(u64::MAX)` and `unwrap_or_else` fallbacks are infallible-fallback patterns, not panics. |
| 4 | Raw SQL | clean | All DB access via `query_as!`/`query!`/`query_scalar!` in `database/mod.rs` + `database/upsert.rs`; no runtime `sqlx::query(_)`. |
| 5 | File size | remediated | `lib.rs` (302) split into thin root + `config.rs` + `result.rs`; `file_bundler.rs` (302) split into `file_bundler/mod.rs` + `file_bundler/extract.rs`. All files now ≤300 lines. |
| 6 | Function size | remediated | `SyncService::sync_all` error-mapping extracted into private `database_failure_result`; all functions well under 75 lines. |
| 7 | Async traits | clean | `#[async_trait]` only on `Job` impls in `jobs/*` — required for the `dyn`-compatible external `Job` trait. No `async fn` in this crate's own public traits. |
| 8 | Typed identifiers | clean | `TenantId`/`UserId`/`ContextId`/`SessionId`/`SourceId`/`ContentId` etc. used throughout; constructed via `Id::new`. Builder `.into()` calls bind `impl Into<TenantId>` generics, not call-site coercion. |
| 9 | Comment standard | clean | Substantive `//!` heads on all modules; no `///` paraphrase walls; no WHAT/narrative inline comments. |
| 10 | No legacy | clean | No shims, dual paths, deprecation stubs, or `Option<T>` migration stubs; no dead code. |
| 11 | Naming | clean | `*Service` / `*SyncJob` / `*Calculator` / `*LocalSync`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | remediated | `sync_all`'s inline `SyncOperationResult` construction folded into the new `database_failure_result` helper. |
| 14 | CHANGELOG accuracy | clean | `[0.9.2]` entries (`SyncOpState` partial-state reporting, skill/user-upsert removals) match current code; skill helpers absent, database sync covers users + contexts only. |

## Summary

Remediated: 5 (file size), 6 (function size), 13 (duplication) — all via the same split/extract refactor. Clean: 11 items. No behavioural changes; no cross-crate signature changes required.
