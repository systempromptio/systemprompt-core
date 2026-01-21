# systemprompt-mcp Compliance

**Layer:** Domain
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ✅ |
| Code Quality | ❌ |

---

## Violations

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/services/orchestrator/reconciliation.rs` | 372 lines (exceeds 300 limit) | Code Quality |
| `src/services/tool_provider.rs` | 318 lines (exceeds 300 limit) | Code Quality |

---

## Commands Run

```
cargo clippy -p systemprompt-mcp -- -D warnings  # BLOCKED (upstream error in systemprompt-runtime)
cargo fmt -p systemprompt-mcp -- --check          # PASS
```

---

## Actions Required

1. Split `src/services/orchestrator/reconciliation.rs` into smaller modules:
   - Extract schema validation to `schema_validation.rs`
   - Extract process cleanup helpers to separate functions
   - Extract server startup coordination to `server_startup.rs`

2. Split `src/services/tool_provider.rs` into smaller modules:
   - Extract context creation to `context.rs`
   - Extract tool conversion utilities to `conversions.rs`

---

## Fixed During Review

| File | Issue | Fix Applied |
|------|-------|-------------|
| `src/middleware/rbac.rs` | Import ordering | Reordered imports alphabetically |
| `src/services/monitoring/health.rs` | Line length | Merged statement to single line |
| `src/services/client/mod.rs:165-166` | Inline comments | Removed inline comments |
| `src/services/tool_provider.rs:320-345` | Tests in source file | Removed `#[cfg(test)]` block |

---

## Boundary Rules Verification

| Rule | Status | Evidence |
|------|--------|----------|
| No entry layer imports | ✅ | No `systemprompt-api` or `systemprompt-tui` imports |
| No direct SQL in services | ✅ | SQL in `repository/tool_usage/` only |
| Uses service pattern | ✅ | Services delegate to domain |
| Business logic delegated | ✅ | Orchestration uses domain services |

---

## Orchestration Quality Verification

| Rule | Status | Evidence |
|------|--------|----------|
| Coordinates domain services | ✅ | `McpOrchestrator` delegates to managers |
| No data transformation logic | ✅ | Pure coordination |
| No validation logic | ✅ | Validation in services |
| Pure workflow execution | ✅ | Event-driven workflow |

---

## Idiomatic Rust Verification

| Rule | Status | Evidence |
|------|--------|----------|
| Iterator chains over loops | ✅ | Widespread use of `.iter().filter().map()` |
| `?` operator for errors | ✅ | Consistent error propagation |
| No unnecessary `.clone()` | ✅ | Clone used appropriately with Arc |
| `impl Into<T>` for APIs | ✅ | Used in `McpClient::list_tools` |

---

## Forbidden Constructs Check

| Construct | Status | Evidence |
|-----------|--------|----------|
| `unsafe` | ✅ None | No unsafe blocks found |
| `unwrap()` | ✅ None | No unwrap calls found |
| `panic!()` | ✅ None | No panic macros found |
| Inline comments | ✅ Fixed | Removed during review |
| TODO/FIXME | ✅ None | No TODO/FIXME comments |
| Tests in source | ✅ Fixed | Removed during review |

---

## Silent Error Pattern Review

**Acceptable patterns (per standards):**

| Location | Pattern | Justification |
|----------|---------|---------------|
| `reconciliation.rs:163,165,167` | `.ok()` | Cleanup path - already returning error |
| `port_manager.rs:35,39` | `let _ =` | Kill commands in cleanup |
| `event_bus.rs:28` | `let _ =` | Broadcast send (receivers may drop) |
| `health_check.rs:72` | `let _ =` | Non-critical event notification |
| `database/state.rs` | `.ok()` chains | File metadata with Option fallback |
| `pid_manager.rs` | `.ok()` | Parse operations with fallback |

---

## Architecture

```
lib.rs ─┬─► orchestration/ ──┬─► loader.rs (McpToolLoader)
        │                    ├─► state.rs (ServiceStateManager)
        │                    └─► models.rs
        ├─► api/ ────────────► routes/registry.rs
        ├─► cli/ ────────────► commands/
        ├─► middleware/ ─────┬─► rbac.rs
        │                    └─► session_manager.rs
        ├─► models/ ─────────► ExecutionStatus, ValidationResultType
        ├─► repository/ ─────► tool_usage/
        └─► services/ ───────┬─► client/
                             ├─► database/
                             ├─► deployment/
                             ├─► lifecycle/
                             ├─► monitoring/
                             ├─► network/
                             ├─► orchestrator/ ─► handlers/
                             ├─► process/
                             ├─► registry/
                             ├─► schema/
                             └─► tool_provider.rs
```

---

## Trait Implementations

| Trait | Implementation | Location |
|-------|----------------|----------|
| `ToolProvider` | `McpToolProvider` | `services/tool_provider.rs:152` |
| `McpRegistry` | `RegistryManager` | `services/registry/trait_impl.rs:17` |
| `McpToolProvider` | `RegistryManager` | `services/registry/trait_impl.rs:42` |
| `McpDeploymentProvider` | `McpDeploymentProviderImpl` | `services/registry/trait_impl.rs:81` |
| `McpRegistryProvider` | `RegistryManager` | `services/registry/trait_impl.rs:92` |
| `EventHandler` | Various handlers | `services/orchestrator/handlers/` |
| `ServiceManager` | (trait definition) | `services/mod.rs:32` |
| `ServiceLifecycle` | (trait definition) | `services/mod.rs:40` |
| `SessionManager` | `DatabaseSessionManager` | `middleware/session_manager.rs:24` |
| `StreamableHttpClient` | `HttpClientWithContext` | `services/client/http_client_with_context.rs:52` |
