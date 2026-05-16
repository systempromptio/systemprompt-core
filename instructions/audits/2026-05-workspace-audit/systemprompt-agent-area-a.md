# Audit — `systemprompt-agent` Area A (`crates/domain/agent/src/repository/`)

Scope: all files under `src/repository/`. Other agents cover the rest of the crate.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only depends on `systemprompt_database`, `systemprompt_traits`, `systemprompt_identifiers`, `systemprompt_models` and intra-crate modules — all downward. |
| 2 | Error model | clean | All public signatures return `RepositoryError` / `AgentError` (`thiserror`); no `anyhow` anywhere in scope. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`/`todo!` in scope. |
| 4 | Raw SQL | clean | All queries use `query!`/`query_scalar!` compile-time macros; no runtime `sqlx::query(_)`. |
| 5 | File size | remediated | `execution/mod.rs` (310) split into `execution/mod.rs` (read paths, 111) + `execution/mutations.rs` (write paths, 214). `task/mutations.rs` (301) split into `task/mutations.rs` (create + status mapping, 100) + `task/state.rs` (state transitions, 219). New `mod` lines added only in `execution/mod.rs` and `task/mod.rs`. |
| 6 | Function size | clean | All functions within the ~75-line guidance after split. |
| 7 | Async traits | clean | No traits defined in scope; no `#[async_trait]`. |
| 8 | Typed identifiers | observation | `apply_notification_status` (`task/mutations.rs` -> `state.rs`, `task/mod.rs`) and `ContextNotificationRepository::insert` take `task_id: &str` / `context_id: &str`. Callers live in `entry/api` and `services/`; converting to typed IDs is a cross-crate signature change — out of scope, flagged for a follow-up. `StepId(pub String)` is a plain model newtype (not `define_id!`); `step_id.into()` at the row-parse boundary is the only construction path and is acceptable. |
| 9 | Comment standard | clean | No `///` paraphrase comments and no stray WHAT `//` comments in scope. `//!` heads added to the two new files; private intra-doc links avoided. |
| 10 | No legacy | clean | No shims/dual paths/deprecation stubs/`Option<T>` migration stubs. |
| 11 | Naming | clean | Repositories are `*Repository`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | Row->model parsing already factored into `parse.rs` / `converters.rs` helpers; no new duplication introduced by the split (`task_state_to_db_string` shared via `super::`). |
| 14 | CHANGELOG | n/a | Not edited (observations only). |

## Summary

Repository scope was standards-clean apart from the two known over-limit files.
Both were split into cohesive sub-modules with identical query semantics and no
behavioural or cross-crate signature changes. `cargo clippy` and `cargo doc`
pass clean for `systemprompt-agent`. One observation (item 8) left for a
follow-up that touches `entry/api`.
