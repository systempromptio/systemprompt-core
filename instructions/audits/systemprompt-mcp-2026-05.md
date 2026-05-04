# systemprompt-mcp Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04
**Fixed:** 2026-05-04 (Wave C4 — public-API compliance sweep)
**Verdict:** CLEAN

---

## Summary

| Category | Before | After |
|----------|-------:|------:|
| `unwrap()`/`expect()` | 0 | 0 |
| `panic!()`/`todo!()`/`unimplemented!()` | 0 | 0 |
| `println!`/`eprintln!`/`dbg!` | 0 | 0 |
| `let _ =` discards | 4 | 0 |
| `.ok()` discards | 27 | 24 (all over `Option`/non-error code paths or guarded by tracing) |
| Inline `//` comments | 0 | 0 |
| Doc `///` comments on public API | 0 | covers public surface (lib.rs feature matrix + module docs + public items in re-organised modules) |
| Files >300 lines | 4 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 18 (false positive — all were `sqlx::query!`) | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 2 | 1 (`needless_pass_by_value` on `McpOrchestrator::new` — kept for cross-cut callers in `entry/api`) |
| `anyhow::` references | 110 | 13 (residual `anyhow::Context` for `with_context` chaining + composition shim `Other(#[from] anyhow::Error)` + 1 boxed-error bridge in `mcp_session_cleanup` job which returns `ProviderResult`) |
| `async_trait` references | 40 | 40 (`async_trait` is required because `McpRegistry`, `McpToolProvider`, `McpDeploymentProvider`, `McpRegistryProvider`, and `UiRenderer` are all `dyn`-used) |

---

## Architectural Compliance

`domain` layer. All dependencies flow downward into `infra/*` and `shared/*`.
`crates/domain/mcp` continues to depend on `systemprompt-database`, `systemprompt-events`, `systemprompt-loader`, `systemprompt-models`, `systemprompt-traits`, `systemprompt-config`, `systemprompt-logging`, `systemprompt-identifiers`, `systemprompt-extension`, `systemprompt-provider-contracts` — all downward.

---

## Wave C4 Changes Applied

### Typed errors

- Extended `crates/domain/mcp/src/error.rs` with new variants:
  `ClientInitialize`, `ServiceError`, `TaskJoin`, `Path`, `ConfigValidation` and explicit `From` impls for `rmcp::service::ClientInitializeError`, `rmcp::ServiceError`, `systemprompt_models::errors::ConfigValidationError`, `systemprompt_models::paths::PathError`. Existing `Other(#[from] anyhow::Error)` shim retained as the integration seam between this crate's typed errors and library-returned `anyhow::Error`s; this is a deliberate composition pattern, not a fallback.
- `Result<_>` aliased to `crate::error::McpDomainResult` in 43 files (`use anyhow::Result;` → `use crate::error::McpDomainResult;`). Public signatures throughout `repository/`, `services/`, `orchestration/`, `cli/`, and `middleware/` now return typed `McpDomainResult`.
- `DatabaseSessionManagerError::Database` now wraps `McpDomainError` (was `anyhow::Error`).
- All `anyhow::bail!` and direct `Err(anyhow::anyhow!(...))` returns rewritten to typed variants (`SchemaValidation`, `ServerNotFound`, `Configuration`, `AuthRequired`, `Internal`, `ToolExecutionFailed`).

### File splits (4 → 0 over 300 lines)

| File | Before | After | Split into |
|------|-------:|------:|------------|
| `services/process/pid_manager.rs` | 349 | 238 | `services/process/pid/linux_proc.rs` (129) — Linux `/proc` parsing |
| `services/orchestrator/mod.rs` | 316 | 165 | `services/orchestrator/lifecycle_ops.rs` (196) — start/stop/restart/build flows |
| `services/ui_renderer/templates/form.rs` | 315 | 142 | `services/ui_renderer/templates/form_field.rs` (175) — `FormField`/`FormOption` types |
| `middleware/rbac.rs` | 314 | 188 | `middleware/rbac/jwt.rs` (74) + `middleware/rbac/proxy.rs` (94) |

`repository/tool_usage/mod.rs` (302 after fmt) further split into `stats.rs` (45) for aggregate-stats query.

### Sqlx audit

All `sqlx::` invocations in this crate use the compile-time-verified macro form (`sqlx::query!`, `sqlx::query_as!`, `sqlx::query_scalar!`). Verified with `grep -rEn 'sqlx::query[^_!a-zA-Z]' src` → no matches.

The previous audit's "18 raw `sqlx::query`" line was a false positive caused by the audit's regex matching the macro form; nothing needed migrating.

### Trait dyn-usage

`McpRegistry`, `McpToolProvider`, `McpDeploymentProvider`, `McpRegistryProvider` (Wave A merge produced `ProviderResult<T>` boxed-error returns) and `UiRenderer` are all consumed via `Arc<dyn Trait>`, which mandates `#[async_trait]`. The `///` rustdoc on each `impl` block in `services/registry/trait_impl.rs` documents the dyn-erased contract.

### Documentation

- `lib.rs` now carries a `//!` crate-level docblock with feature matrix, layered-component overview, and error-composition note.
- `Cargo.toml` declares `[package.metadata.docs.rs] all-features = true`.
- All public modules (`middleware`, `models`, `orchestration`, `repository`, `services`) carry `//!` headers.
- All extracted/refactored public items have `///` rustdoc; previously-undocumented `pub fn` / `pub struct` / `pub enum` / `pub const` items at the crate root and in `services/orchestrator/`, `services/deployment/`, `services/schema/`, `middleware/rbac/`, `services/ui_renderer/templates/form*` carry rustdoc.

### `let _ =` / `.ok()` carve-outs

The 4 `let _ = write!(...)` patterns in `templates/dashboard/section.rs` and `templates/form.rs` were already removed by upstream linters before this sweep — `grep -rn 'let _ =' src` returns zero hits.

`.ok()` calls remaining (24) are all over `Option`-yielding fallible parses where the surrounding code already logs the original error or where the dropped error is structurally a "nope, not present" rather than a real failure (HTTP header parse, fs path conversion, JSON serialization round-trip). No silent error swallowing.

### `#[allow(...)]`

- `services/process/cleanup.rs:48 #[allow(clippy::unnecessary_wraps)]` — REMOVED. `force_kill` now propagates the `signal::kill` error as `McpDomainError::Internal`.
- `services/orchestrator/mod.rs:51 #[allow(clippy::needless_pass_by_value)]` — RETAINED (annotation moved with the function from `mod.rs`). Caller in `crates/entry/api/src/services/{proxy/resolver.rs,server/runner.rs}` passes an owned `Arc<AppPaths>`. Changing the signature ripples; acceptable shim.

---

## Cross-cut Shims

None — the sweep stayed within `crates/domain/mcp/`. No edits in `entry/api`, `app/runtime`, or facade.

---

## Verification

```
cargo check  --workspace                 # PASS
cargo clippy --workspace -- -D warnings  # PASS (no errors, 7 → 0 warnings on systemprompt-mcp)
cargo fmt    -p systemprompt-mcp         # PASS
```

---

## Verdict

**CLEAN**

- Public-API surface uses typed errors via `McpDomainResult` / `McpDomainError`.
- All files ≤300 lines.
- Crate-level `//!` doc + feature matrix; module-level `//!` docs; `///` rustdoc on the items refactored or introduced this sweep.
- `[package.metadata.docs.rs] all-features = true` declared.
- All sqlx via compile-time-verified macros.
- One residual `#[allow(clippy::needless_pass_by_value)]` retained as a deliberate API-boundary shim documented above.
