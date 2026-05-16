# Audit — systemprompt-cli area 3: `commands/infrastructure/`

Scope: `crates/entry/cli/src/commands/infrastructure/` (db, jobs, logs, services).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Entry crate; only depends downward on domain/infra services and `crate::shared`. No sideways/circular deps. |
| 2 | Error model | clean | `anyhow::Result` throughout — permitted in entry binary. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`todo!`/`unimplemented!`/`dbg!` in any `.rs` file. |
| 4 | Raw SQL | clean | No `sqlx::query*` calls; all DB access routed through domain/infra services and the `Database` query API. |
| 5 | File size | clean | Largest file `db/schema.rs` at 298 lines; all under the 300-line limit. |
| 6 | Function size | remediated | `execute_ai_trace` (105 lines) split: extracted cohesive `ai_summary` + `build_trace_output` helpers. Remaining 76–93-line fns (`db/mod::execute*`, `services/serve::execute_with_events`, `logs/search`, `logs/trace/show`, `logs/request/*`) are linear dispatch/data-gathering flows within the soft ~75-line guidance — left as-is, no safe cohesive split. |
| 7 | Async traits | clean | No trait definitions in scope; no `#[async_trait]`. |
| 8 | Typed identifiers | clean | Typed IDs (`TaskId`, `ContextId`, `TraceId`) used; constructed via `::new`; no `.into()`/`::from()` at call sites. |
| 9 | Comment standard | clean | No `///` in `.rs` files (banned in entry); no `//!` heads (correct for entry); no inline `//` WHAT-comments. |
| 10 | No legacy | clean | No shims, dual paths, `Option<T>` stubs, or commented-out code. |
| 11 | Naming | clean | No `*Manager` types; consumes `*Service` types from libraries. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | `helpers.rs` files (`jobs`, `db`) hold genuinely cohesive shared helpers (cron formatting, byte formatting, fuzzy table matching) — not padding. |
| 14 | CHANGELOG | clean | Not edited; observations only. |

## Summary
Area is in strong shape. One remediation: `logs/trace/ai_trace_display.rs` `execute_ai_trace` reduced from 105 lines by extracting two cohesive pure helpers (`ai_summary`, `build_trace_output`) — no behavioural change. Verified clean: `cargo clippy -p systemprompt-cli --all-targets --all-features -D warnings` and `cargo doc -p systemprompt-cli --no-deps`.
