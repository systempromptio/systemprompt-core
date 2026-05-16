# Audit — systemprompt-mcp (Area B)

Scope: `crates/domain/mcp/src/services/{client,registry,monitoring,lifecycle,tool_provider,network,schema,database}/`

1. **Layering** — clean. Deps flow downward (database/loader/traits/models); no upward or cross-domain imports.
2. **Error model** — clean. All public signatures use `McpDomainResult` / typed `thiserror` enums; no `anyhow`.
3. **No panics** — clean. No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in scope.
4. **Raw SQL** — clean. All DB access goes through `ServiceRepository`; no raw `sqlx::query`.
5. **File size** — clean. Largest file is `monitoring/health.rs` at 286 lines, under the 300 limit.
6. **Function size** — clean. No function exceeds the ~75-line guidance.
7. **Async traits** — clean. `#[async_trait]` appears only on impls of external `dyn`-compatible provider traits (`McpRegistry`, `McpToolProvider`, `ToolProvider`); the macro is required by the trait definitions.
8. **Typed identifiers** — clean. No raw `String` ID fields in scope; `McpServerId`/`ContextId` constructed via explicit constructors.
9. **Comment standard** — remediated. Added substantive `//!` heads to the eight in-scope `mod.rs` files (`client`, `registry`, `monitoring`, `lifecycle`, `tool_provider`, `network`, `schema`, `database`), which previously had none. No `///` paraphrase or stale narration found.
10. **No legacy** — remediated. Removed dead `update_service_state` fn (no callers) and the dead `_startup_time`/`startup_time` parameter threaded through `state::register_service`, `DatabaseManager::register_service`, and `lifecycle::startup::start_server` (value computed then discarded).
11. **Naming** — observed, not remediated. Pervasive `*Manager` types (`RegistryManager`, `MonitoringManager`, `NetworkManager`, `DatabaseManager`, `LifecycleManager`) violate the `*Service` convention, but every type is referenced from out-of-scope files (`services/mod.rs`, `orchestrator/**`, `lib.rs`, `orchestration/loader.rs`). Renaming cannot be done within scope without editing forbidden files; flagged for a crate-wide pass.
12. **Tests location** — clean. No inline `#[cfg(test)] mod tests`.
13. **Local duplication** — clean. No material duplication warranting extraction.
14. **CHANGELOG** — not edited (observations only).

### Additional remediation
- `monitoring/status.rs`: `get_service_status` swallowed health-probe errors via `Err(_) => Ok(...)`; added a `tracing::debug!` log before falling back to the unreachable status (§6 silent-error rule).

### Verification
- `SQLX_OFFLINE=true cargo clippy -p systemprompt-mcp --all-targets --all-features -- -D warnings` — clean.
- `SQLX_OFFLINE=true cargo doc -p systemprompt-mcp --no-deps` — clean.
