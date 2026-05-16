# Audit — systemprompt-cli area 5: commands/analytics/

Scope: `crates/entry/cli/src/commands/analytics/**` (42 source files).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Entry → runtime (`AppContext`/`DatabaseContext`) → domain `systemprompt-analytics`; no sideways/circular deps. |
| 2 | Error model | clean | `anyhow::Result` throughout — permitted in entry crate. |
| 3 | No panics | clean | No `unwrap()`/`expect()`/`panic!`/`dbg!`/`todo!`; only `unwrap_or(...)` defaults. |
| 4 | Raw SQL | clean | No `sqlx` usage in CLI; all data access via `*AnalyticsRepository` in the analytics domain crate. |
| 5 | File size | clean | Largest is `overview.rs` at 254 lines; all under the 300-line limit. |
| 6 | Function size | clean | Longest functions (`fetch_overview_data`, `export_overview_csv`) ~75 lines, cohesive; no `*_helpers.rs` padding. |
| 7 | Async traits | clean | No trait definitions; plain `async fn` command entrypoints. |
| 8 | Typed identifiers | clean | Analytics commands filter by time-range/name, not entity IDs; no raw `String` IDs. `.into()` occurrences are `impl Into<String>` builders, not typed-ID call sites. |
| 9 | Comment standard | clean | No `///` rustdoc; no narrative `//` WHAT-comments. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` migration stubs; `execute`/`execute_with_pool` are two genuine entrypoints (own-context vs shared pool), not a migration pair. |
| 11 | Naming | clean | No `*Manager`; consumes `*Repository`/`CliService` from domain/infra. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | Shared formatting/CSV/time logic already factored into `analytics/shared/`. |
| 14 | CHANGELOG | n/a | Observation only; `CHANGELOG.md` not edited. |

Result: all 14 items clean — no remediation required.
