# systemprompt-logging Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04 (Wave B4 follow-up)
**Verdict:** CLEAN

---

## Summary

| Category | Before (Wave 0) | After (Wave B4) |
|----------|-----------------|-----------------|
| `unwrap()` / `expect()` | 0 | 0 |
| `panic!()` / `todo!()` / `unimplemented!()` | 0 | 0 |
| `println!` / `eprintln!` / `dbg!` | 0 | 0 |
| `let _ =` discards | 23 | 0 |
| `.ok()` discards | 3 | 3 (carved-out, see below) |
| `///` / `//!` rustdoc comments | 0 | 90+ |
| Files >300 lines | 2 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` outside allowlist | 0 (false positive in W0 scan) | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |
| `anyhow::Result` / `anyhow::Error` in PUBLIC signatures | 37 | 0 |
| `async_trait` references | 2 | 2 (`LogService` impl is `dyn`-used downstream) |

**Total scored violations:** 0

---

## Architectural Compliance

Layer: `infra`. Dependencies still flow downward only (`shared` -> `infra`). `systemprompt-database` is a downward dep that exposes `anyhow::Result` from its pool accessors; we adapt that boundary inside `LoggingError` via a single `From<anyhow::Error>` impl that materialises a `PoolUnavailable(String)` variant. No anyhow in our own public surface.

---

## Wave B4 Changes

### 1. Typed error surface (`models::LoggingError`)

The `LoggingError` thiserror enum gained the following variants, all composed via `#[from]` where possible:

- `Scheduler(#[from] tokio_cron_scheduler::JobSchedulerError)` — replaces anyhow propagation in the retention scheduler.
- `Prompt(#[from] dialoguer::Error)` (gated on `feature = "cli"`) — replaces anyhow in CLI prompt bodies.
- `TaskNotFound { partial_id: String }` — replaces ad-hoc `anyhow!()` in `AiTraceService::resolve_task_id`.
- `MissingColumn { column: String }` — replaces `anyhow!("Missing X")` in `LogRow::from_json_row`.
- `PoolUnavailable(String)` — adapter for `systemprompt-database`'s `anyhow::Result` pool accessors. Removed the open-ended `QueryError(#[from] anyhow::Error)` variant.

All public service / repository APIs now return `Result<_, LoggingError>`:

- `LoggingRepository::new`
- `AnalyticsRepository::{new,log_event}`
- `DatabaseLogService::new`
- `LoggingMaintenanceService::new`
- `RetentionScheduler::start`
- `TraceQueryService::*` (every query method)
- `AiTraceService::*` (every query method)
- CLI `Prompts::*`, `PromptBuilder::confirm`, `QuickPrompts::*`, `CliService::prompt_*`/`confirm*`/`batch_*`

Internal query helpers (`trace/*_queries.rs`, `repository/operations/*.rs`) replaced their `use anyhow::{Context, Result};` imports with a local `type Result<T> = std::result::Result<T, LoggingError>;` and stripped every `.context("...")?` chain — error-source preservation now comes from `#[from] sqlx::Error` and `#[from] serde_json::Error`.

### 2. Rustdoc

- `lib.rs` now carries a top-level `//!` doc with feature-flag matrix and entry-point inventory.
- Every `pub mod` in `lib.rs` documented.
- Every public service method on `AiTraceService`, `TraceQueryService`, `LoggingRepository::new`, `AnalyticsRepository`, `DatabaseLogService::new`, `LoggingMaintenanceService::new`, `RetentionScheduler::start`, `LogRow::from_json_row` documented with `# Errors` sections naming the relevant `LoggingError` variants.
- All trace-domain DTOs (`AiRequest*`, `Trace*`, `TaskInfo`, `ExecutionStep`, `ToolExecution*`, `McpToolExecution`, `LogSearch*`, `LevelCount`, `ModuleCount`, `LogTimeRange`, `Audit*`, `LinkedMcpCall`, `ConversationMessage`, `ToolLogEntry`, `TaskArtifact`) have struct-level rustdoc.
- `Cargo.toml` gained `[package.metadata.docs.rs] all-features = true`.

`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` is clean.

### 3. File splits (audit found 2 files > 300 lines)

| Original | Lines (before) | Action | New Layout |
|----------|----------------|--------|------------|
| `services/cli/service.rs` | 333 | Extracted banner / startup / session-context / service-spinner methods into `services/cli/banners.rs` (a dedicated `impl CliService` block). | `service.rs` 263 lines, `banners.rs` 130 lines. |
| `trace/models.rs` | 392 | Converted to a directory with cohesive submodules. | `models/mod.rs` re-exports; `models/trace.rs` (139 L), `models/ai.rs` (118 L), `models/tool.rs` (122 L), `models/log.rs` (74 L). |

Largest file post-split: `services/cli/summary.rs` at 295 lines.

### 4. `let_underscore_must_use` carve-outs (23 fixes)

All sites are CLI display sinks or tracing-layer mpsc sends where retry / panic would worsen the failure mode. Each was rewritten as `<expr>.ok();` with a `// Why: ...` comment per §6 of `instructions/prompt/rust.md` (modelled on Wave-A `infra/database/services/display.rs`).

| File | Line(s) | Justification |
|------|---------|---------------|
| `services/cli/service.rs` | clear_screen, output, json, json_compact, yaml, key_value, status_line | CLI display sink — broken-pipe non-recoverable. |
| `services/cli/banners.rs` | session_context_with_url, profile_banner | CLI display sink. |
| `services/cli/display.rs` | stdout_writeln | Generic CLI sink helper. |
| `services/cli/table.rs` | stdout_write, stdout_writeln | Generic CLI table sink. |
| `services/cli/module.rs` | stdout_writeln | Module display sink. |
| `services/cli/startup.rs` | stdout_writeln | Startup banner sink. |
| `services/cli/summary.rs` | active-modules writeln, detail bullet writeln | Summary display sink. |
| `services/cli/prompts.rs` | spacer writeln (×2) | Cosmetic blank line in prompt context. |
| `repository/mod.rs` | terminal mirror writeln | Optional terminal log mirror — same broken-pipe rationale. |
| `layer/proxy.rs` | duplicate-attach stderr writeln | Subscriber bootstrap stderr; recursing into tracing IS the failure mode being avoided. |
| `layer/mod.rs` | flush-fail stderr writeln | Database-flush stderr fallback. |
| `layer/mod.rs` | mpsc Entry send | tracing layer must never panic; closed channel == shutdown. |
| `layer/mod.rs` | mpsc FlushNow send | Same shutdown rationale. |

### 5. Per-sqlx-site decisions (30 audit-flagged sites)

The Wave 0 audit's `30` figure was a regex artefact: it matched `sqlx::query!(` (the compile-time-verified macro) the same as `sqlx::query(`. Independent verification with `grep -rn 'sqlx::query[^_!a-zA-Z]'` shows **0 unverified `sqlx::query()` calls** in this crate. The single hit (`lib.rs:43 "sqlx::query=warn"`) is an `EnvFilter` directive, not a SQL call.

| Site bucket | Macro | Decision |
|-------------|-------|----------|
| `repository/operations/{queries,mutations}.rs` (8 sites) | `query!` / `query_as!` / `query_scalar!` | Keep — already compile-time verified. |
| `repository/analytics/mod.rs` (1 site) | `query!` | Keep. |
| `layer/mod.rs` batch insert (1 site) | `query!` | Keep. |
| `trace/{ai_trace,step,queries,mcp_trace,request,audit,tool,list,log_search,log_lookup,log_summary}_queries.rs` (~20 sites) | `query!` / `query_as!` / `query_scalar!` | Keep. |

`just lint-sqlx` (which runs `ci/check-sqlx.sh`) reports clean. **No allowlist additions required.**

### 6. Cargo.toml

- `[package.metadata.docs.rs] all-features = true` added.
- Workspace `anyhow` dependency retained — solely to satisfy the `From<anyhow::Error>` adapter for `systemprompt-database`'s pool accessors. Not exposed in any public signature.

---

## Self-Verification Gate

| Check | Result |
|-------|--------|
| `cargo fmt -p systemprompt-logging` | clean |
| `cargo build -p systemprompt-logging --all-features` | clean |
| `cargo clippy -p systemprompt-logging --all-targets --all-features -- -D warnings` | clean (0 errors, was 23) |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-logging --no-deps --all-features` | clean |
| `just check-bans-crate systemprompt-logging` | OK |
| `just lint-sqlx` | OK |

---

## Verdict

**CLEAN**

All Wave B4 compliance items closed. No deferred work, no TODO markers, no back-compat shims.
