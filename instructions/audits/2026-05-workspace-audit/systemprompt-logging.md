# Audit — `systemprompt-logging` (`crates/infra/logging/`)

Date: 2026-05-15. Workspace audit, 14-item checklist.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only on `shared/*` (`systemprompt-traits`, `-identifiers`, `-extension`) and `infra/database`; no upward/cross-layer deps. |
| 2 | Error model | clean | `LoggingError` is a `thiserror` enum in `models/log_error.rs`; no `anyhow` in crate or signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`. `println!`/`eprintln!` confined to allowlisted `services/cli/**` sinks. |
| 4 | Raw SQL | clean | No `sqlx::query(_)` calls; all access via compile-time `query!`/`query_as!`/`query_scalar!` macros. |
| 5 | File size | clean | Largest source file is 290 lines (`services/cli/summary.rs`); all under the 300-line limit. |
| 6 | Function size | clean | No functions exceed the ~75-line guidance; query files are flat collections of small functions. |
| 7 | Async traits | clean | `#[async_trait]` on the `LogService` impl mirrors the trait declared in `shared/traits` (out of scope); not a logging-crate defect. |
| 8 | Typed identifiers | remediated | Three `.map(TaskId::from)` call-site uses rewritten to `TaskId::new` (`ai_trace_service.rs`, `service.rs`). Remaining `&str` IDs on `trace/` query/service signatures are a public-API change deferred to a coordinated cross-crate pass. |
| 9 | Comment standard | clean | Substantive `//!` heads; no `///` paraphrase smell; the only inline `//` blocks encode a genuine non-obvious WHY (non-UUID `context_id` skip). |
| 10 | No legacy | clean | No backwards-compat shims, dual code paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `*Service`/`*Repository`/`*Layer`/`*Extension`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No notable repeated logic; query helpers are intentionally distinct per query shape. |
| 14 | CHANGELOG accuracy | clean | Entries accurately describe the code (`ProxyDatabaseLayer`, `ensure_subscriber`, `layer/proxy`). Latest entry `0.9.2` lags the `0.10.1` workspace version, consistent with sibling infra crates — a release-process item, not an audit defect. |

## Remediation summary

- Item 8: replaced three `TaskId::from` call-site constructions with the canonical `TaskId::new` form.

## Deferred (out of single-crate scope)

- Item 8: `trace/` query functions and `TraceQueryService`/`AiTraceService` public methods accept raw `&str` for `trace_id`/`request_id`/`id`. Converting these to typed IDs is a public-API change rippling into CLI callers; it must run as a coordinated workspace change, not an isolated single-crate fix.
