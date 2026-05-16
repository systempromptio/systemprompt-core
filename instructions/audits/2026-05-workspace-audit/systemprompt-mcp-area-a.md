# Audit — systemprompt-mcp (Area A)

Scope: `services/ui_renderer/`, `services/orchestrator/`, `services/process/`,
`services/deployment/`, `services/auth.rs`, `services/providers.rs`, `services/mod.rs`.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only downward deps (`systemprompt_models`, `systemprompt_config`, `systemprompt_loader`, `systemprompt_traits`); no upward/cross-layer use. |
| 2 | Error model | clean | All public fns return `McpDomainResult`/`thiserror` `McpDomainError`; no `anyhow` in signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in scope; all logging via `tracing`. |
| 4 | Raw SQL | clean | No SQL in scope — these services do not touch the database directly. |
| 5 | File size | clean | Largest in-scope file is 284 lines (`dashboard/section.rs`); none exceed 300. |
| 6 | Function size | remediated | `process/spawner.rs::spawn_server` was 128 lines; extracted `configure_environment` (env-var setup) behind a `SpawnEnvironment<'a>` param struct (keeps param count ≤ 5). No behaviour change. |
| 7 | Async traits | remediated | `UiRenderer` and `EventHandler` are genuinely `dyn`-compatible (`Arc<dyn ...>` in registry/event-bus); added the required WHY comment documenting the `#[async_trait]` reason on both trait definitions. Impl-site `#[async_trait]` follows the trait — correct. |
| 8 | Typed identifiers | observation | `orchestrator/events.rs`, `orchestrator/mod.rs`, `deployment/mod.rs` use raw `&str`/`String` for MCP server *names*. A typed `McpServerId` exists. Converting would ripple through `McpEvent` (re-exported via `lib.rs`) and out-of-scope callers (`lifecycle/`, `monitoring/`); deferred — exceeds Area-A scope and the rule is a reviewer convention, not a lint. |
| 9 | Comment standard | clean | No `///` paraphrase anywhere in scope; no inline WHAT/narrative comments. Added substantive `//!` heads to `deployment/mod.rs`, `process/mod.rs`, `ui_renderer/mod.rs`, `ui_renderer/templates/mod.rs`, `orchestrator/handlers/mod.rs`, which previously lacked them. |
| 10 | No legacy | clean | No shims/dual paths/`Option<T>` migration stubs in scope. (`ServiceManager`/`ServiceLifecycle` traits in `mod.rs` have zero implementors — noted under #11; not strictly legacy code.) |
| 11 | Naming | observation | `ProcessManager` (`process/mod.rs`), `DeploymentService` ok. `ServiceManager` trait + `McpManager` alias live in `mod.rs`; `DatabaseManager`/`LifecycleManager`/`MonitoringManager`/`NetworkManager`/`RegistryManager` are re-exports from out-of-scope modules. Renaming `ProcessManager`/`ServiceManager`/`McpManager` is a public-API change touching `lib.rs` (out of scope) and external callers — deferred. `pid_manager` is a module name, not a type. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests` in scope. |
| 13 | Local duplication | clean | No notable copy-paste; per-server config lookups in `deployment/mod.rs` already funnel through `load_config()` + `missing_deployment()`. |
| 14 | CHANGELOG | n/a | Not edited — owned by another agent. |

## Remediations applied
- `process/spawner.rs`: extracted `configure_environment` + `SpawnEnvironment<'a>` from the 128-line `spawn_server`.
- `ui_renderer/mod.rs`, `orchestrator/handlers/mod.rs`: documented the `#[async_trait]`/`dyn`-compat rationale on the `UiRenderer` and `EventHandler` traits.
- Added `//!` module heads to five `mod.rs` files that lacked them.

## Observations for follow-up (out of Area-A scope)
- Adopt `McpServerId` across `McpEvent` variants and orchestrator/deployment service args.
- Rename `*Manager` types/aliases (`ProcessManager`, `ServiceManager`, `McpManager`, and the cross-module re-exports) to `*Service`/`*Orchestrator`.
- `ServiceManager` / `ServiceLifecycle` traits in `services/mod.rs` have no implementors — candidates for removal.
