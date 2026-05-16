# Audit — systemprompt-agent Area C

Scope: `crates/domain/agent/src/services/{agent_orchestration,mcp,external_integrations}/`

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only intra-crate + downward deps (database, identifiers, models, traits, rmcp). |
| 2 | Error model | clean | `thiserror`-derived `OrchestrationError`/`IntegrationError`/`ArtifactError`; no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`; only `unwrap_or`/`unwrap_or_else` (non-panicking). |
| 4 | Raw SQL | clean | No SQL in scope; all DB access via repositories (`AgentServiceRepository`, `TaskRepository`, `ContextRepository`). |
| 5 | File size | clean | Largest file 273 lines (`database.rs`); all under the 300-line limit. |
| 6 | Function size | clean | Largest is `ensure_task_exists` (~143 lines) but cohesive single-flow context resolution; no padding files. Acceptable; no behavioural-safe split available without churn. |
| 7 | Async traits | clean | No trait definitions in scope; no `#[async_trait]`. |
| 8 | Typed identifiers | clean | `AgentId`/`TaskId`/`ContextId`/`WebhookEndpointId` etc. used; constructed via `::generate()`; `.into()` occurrences are error conversions (`RowParseError` -> `ArtifactError`), not ID construction. |
| 9 | Comment standard | clean | `//!` heads present and substantive on module files; no `///` paraphrase comments anywhere; inline `//` absent. |
| 10 | No legacy | clean | No shims/dual paths/deprecation stubs/`Option<T>` migration stubs. |
| 11 | Naming | observation (not remediated) | `PortManager` / `port_manager` module is a `*Manager` name. NOT remediated: the type is re-exported and consumed by `crates/entry/api/src/services/server/lifecycle/agents.rs` and `crates/tests/unit/domain/agent/...`; renaming is a cross-crate signature change forbidden by the audit's Step 4 scope rule. Flagged for a coordinated rename. `ToolResultHandler`/`AgentMonitor`/`AgentReconciler`/`AgentOrchestrator`/`McpToA2aTransformer`/`WebhookService` all compliant. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`; unit tests live in `crates/tests/unit/domain/agent/`. |
| 13 | Local duplication | clean | `get_process_info(...).map_err(trace).ok().flatten().map_or_else(...)` pattern appears twice in `port_manager/mod.rs` but in distinct error-message contexts; extraction would not reduce meaningful logic. No actionable duplication. |
| 14 | CHANGELOG | clean | Not edited. |

## Outcome

13 of 14 items clean. Item 11 (`PortManager` naming) is a genuine violation but cannot be
remediated within the audit's scope constraint (cross-crate signature change); recorded as an
observation for a coordinated follow-up. No code changes were required in this area.
