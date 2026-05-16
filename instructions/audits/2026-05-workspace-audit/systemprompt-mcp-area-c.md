# Audit: systemprompt-mcp — Area C

Scope: `crates/domain/mcp/` excluding `src/services/`. Covers `repository/`,
`middleware/`, `orchestration/`, `cli/`, `jobs/`, `models/`, and root files
(`lib.rs`, `capabilities.rs`, `error.rs`, `extension.rs`, `progress.rs`,
`resources.rs`, `response.rs`, `schema.rs`, `state.rs`, `tool.rs`) plus
`CHANGELOG.md`.

1. **Layering** — clean. Depends only on infra/shared crates; no upward deps.
2. **Error model** — clean. `domain_error!`-generated `McpDomainError` (thiserror); no `anyhow` in public signatures.
3. **No panics** — clean. No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`. `to_str().unwrap_or(...)` and the `AuthResult::expect_authenticated` method name are not panics.
4. **Raw SQL** — clean. All repositories use `query!`/`query_scalar!`; no runtime `sqlx::query(_)`.
5. **File size** — clean. Largest in-scope file is 297 lines (`middleware/session_manager.rs`); all under 300.
6. **Function size** — clean. All functions within ~75-line guidance.
7. **Async traits** — remediated. `McpToolHandler` (`tool.rs`) used `#[async_trait]` despite having associated types (never `dyn`-compatible) and no documented reason; converted to native `async fn` via `-> impl Future + Send`. `Job` impl in `jobs/mcp_session_cleanup.rs` correctly mirrors the foreign `#[async_trait]` trait — left as-is.
8. **Typed identifiers** — clean. Struct fields and service args use typed IDs (`SessionId`, `UserId`, `McpExecutionId`, `ArtifactId`, `ContextId`, etc.); construction via `::new`/`::try_new`/`::generate`. `.into()` occurrences are on plain strings (column names, host strings), not entity IDs.
9. **Comment standard** — remediated. Three module heads (`middleware/mod.rs`, `orchestration/mod.rs`, `repository/mod.rs`) carried a non-substantive placeholder `//!` ("Publicly re-exported submodule…"); replaced with substantive descriptions of each module's purpose and surface. `lib.rs` head already states purpose/surface/feature-matrix/error-model.
10. **No legacy** — clean. No shims, dual paths, deprecation stubs, or `Option<T>` migration stubs.
11. **Naming** — clean within scope. `*Service`/`*Repository`/`*Orchestrator`/`*Handler` used appropriately; no `*Manager` introduced here. (`ServiceStateManager`, `RegistryManager`, `McpManager` live in `src/services/` — other agents' scope.)
12. **Tests location** — clean. No inline `#[cfg(test)] mod tests`.
13. **Local duplication** — clean. `McpArtifactRecord` row-mapping recurs in `find_by_id`/`list_by_server` but is a trivial field projection; extracting a helper adds no value. No actionable duplication.
14. **CHANGELOG accuracy** — clean. The `[0.10.2]` entry (resilience layer, `Timeout`/`CircuitOpen`/`DependencyUnavailable`, `classify`) matches `error.rs`. `[0.1.18]` (request logging middleware, proxy-verified auth, stale session cleanup) matches `lib.rs`/`middleware/`/`jobs/`. No new in-scope changes require a CHANGELOG entry (standards-only edits).

## Remediations applied

- `tool.rs`: removed `#[async_trait]` from `McpToolHandler`; `handle` is now native `async fn` (`-> impl Future + Send`), `async_trait` import dropped.
- `middleware/mod.rs`, `orchestration/mod.rs`, `repository/mod.rs`: replaced placeholder `//!` heads with substantive module documentation.

## Verification

- `SQLX_OFFLINE=true cargo clippy -p systemprompt-mcp --all-targets --all-features -- -D warnings` — clean.
- `SQLX_OFFLINE=true cargo doc -p systemprompt-mcp --no-deps` — clean.
