# Audit: systemprompt-generator (crates/app/generator)

Date: 2026-05-16. Workspace version 0.10.2.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Deps flow downward only (shared/infra/domain + app/sync); no upward deps. |
| 2 | Error model | clean | `thiserror`-derived `PublishError`; no `anyhow` anywhere. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in `src/`. |
| 4 | Raw SQL | clean | No `sqlx::query` runtime calls; crate has no direct SQL. |
| 5 | File size | clean | Largest file 285 lines (`sitemap/generator.rs`), under 300. |
| 6 | Function size | clean | No function exceeds the ~75-line guidance. |
| 7 | Async traits | clean | `#[async_trait]` only on `Job`/RSS/sitemap provider impls — required to match external `dyn`-compatible trait contracts. |
| 8 | Typed identifiers | clean | `SourceId::new` used correctly; `String` fields are XML/URL value types, not entity IDs; `.into()` calls are `String` conversions. |
| 9 | Comment standard | clean | Substantive `//!` heads; no `///` paraphrase walls; no inline WHAT-comments. |
| 10 | No legacy | remediated | Removed empty 0-byte `src/api.rs` — orphaned dead file not declared as a module in `lib.rs`. |
| 11 | Naming | clean | No `*Manager`; `*Job`/`*Provider`/`*Orchestrator` used appropriately. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No notable copy-paste; provider-config loaders are intentionally per-module. |
| 14 | CHANGELOG accuracy | clean | Entries match code state; latest 0.9.2 entry accurate. Version-lag to 0.10.2 is a workspace-wide pattern (sync/runtime siblings identical), out of scope for a single-crate audit. |

Result: 13 clean, 1 remediated.
