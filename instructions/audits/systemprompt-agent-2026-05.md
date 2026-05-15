# systemprompt-agent §3a Public-API Hygiene Sweep

**Layer:** Domain
**Audited:** 2026-05-04 (Wave C5)
**Supersedes:** `agent-2026-04.md` (the earlier "CLEAN" exemplar; predates §3a)
**Verdict:** CLEAN

---

## Summary

| Category | Status | Notes |
|----------|--------|-------|
| Public-API anyhow | :white_check_mark: | All public function signatures typed |
| Module rustdoc (`//!`) | :white_check_mark: | Top-level + every file split has a header |
| Public-item rustdoc (`///`) | :large_blue_diamond: | Public surface in `lib.rs`, `error.rs`, repository facades, server, file splits — full coverage. Deep DTOs in `models/a2a/*` and `models/web/*` retain previous `Debug` derives without per-item `///` lines (see "Residual"). |
| File-size <=300 | :white_check_mark: | 0 files over 300 (was 6) |
| sqlx allowlist | :white_check_mark: | 0 raw `sqlx::query()` sites — original "64 sqlx" baseline was a false positive |
| Native `async fn` traits | :white_check_mark: | No `async_trait` on non-`dyn` traits introduced or required by this sweep |
| `let _` / `.ok()` carve-outs | :white_check_mark: | Inherited from 2026-04 exemplar; no regressions |

---

## §3a — Public-API anyhow elimination

| Site | Before | After |
|------|--------|-------|
| `repository::A2ARepositories::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::TaskRepository::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::ContextRepository::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::content::ArtifactRepository::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::task::constructor::TaskConstructor::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::execution::ExecutionStepRepository::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::content::PushNotificationConfigRepository::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `repository::agent_service::AgentServiceRepository::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `services::context_provider::ContextProviderService::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `services::a2a_server::server::Server::new` | `anyhow::Result<Self>` | `Result<Self, AgentError>` |
| `services::a2a_server::server::Server::reload_config` | `anyhow::Result<()>` | `Result<(), AgentError>` |
| `services::a2a_server::server::Server::run` | `anyhow::Result<()>` | `Result<(), AgentError>` |
| `services::a2a_server::server::Server::run_with_shutdown` | `anyhow::Result<()>` | `Result<(), AgentError>` |
| `services::a2a_server::streaming::broadcast::broadcast_artifact_created` | `Result<(), anyhow::Error>` | `Result<(), AgentError>` |

### `error.rs` extensions

Added typed variants to `AgentError`:
- `Init(String)` — pool/repository init failure
- `Server(String)` — A2A HTTP server lifecycle errors
- `Webhook(String)` — outbound webhook broadcast failures
- `Config(String)` — global config load failures
- `Http(#[from] reqwest::Error)` — outbound HTTP

Bridge impls:
- `impl From<AgentError> for systemprompt_traits::RepositoryError`
- `OrchestrationError::Agent(#[from] AgentError)`

The pre-existing `AgentError::Other(#[from] anyhow::Error)` was retained as the
documented escape hatch for upstream crates whose APIs still hand out
`anyhow::Error`. New code routes through typed variants.

### Anyhow line count

| | Lines |
|-|------:|
| Before | 102 |
| After (internal `Context` / private `?` only) | 144* |

*The count grew because the typed-error bridge pulls in additional
`map_err(\|e\| AgentError::Init(e.to_string()))` call sites and the audit-doc
comments themselves mention `anyhow`. Public-signature anyhow count is **0**.

---

## File splits (>300 line rule)

| File (before, lines) | Split into |
|----------------------|-----------|
| `services/agent_orchestration/process.rs` (356) | `process/{mod,command,signals}.rs` |
| `services/mcp/task_helper.rs` (329) | `task_helper/{mod,completion,messages}.rs` |
| `services/agent_orchestration/port_manager.rs` (327) | `port_manager/{mod,probe}.rs` |
| `services/a2a_server/streaming/event_loop.rs` (323) | `event_loop.rs` + `event_loop_lifecycle.rs` |
| `services/a2a_server/streaming/initialization.rs` (318) | `initialization.rs` + `initialization_steps.rs` |
| `repository/execution/mod.rs` (323 after doc) | `execution/{mod,parse}.rs` |

Largest file post-split: 289 (`repository/execution/mod.rs`).

---

## Sqlx audit

`grep -E 'sqlx::query[^_!a-zA-Z]'` against `crates/domain/agent/src` returns
**0** matches. The baseline number "64" came from non-strict matching that
counted every macro invocation. No raw, unverified `sqlx::query()` sites
exist; allowlist edit not required.

---

## docs.rs metadata

`crates/domain/agent/Cargo.toml`:

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

`lib.rs` includes a feature-flag matrix. The crate currently has no Cargo
features beyond defaults (the facade crate `systemprompt` is what gates
inclusion); this is documented in the matrix.

---

## Residual

- `services::a2a_server::processing::strategies::planned::tool_execution::HandleToolCallsParams.planning_tracked` and the analogous field in `direct_response.rs` carry an `anyhow::Error` as **data** rather than as a return type. These are public struct fields used only by the planned-strategy internals reachable from `services` (not re-exported through `lib.rs`). Wave A merge owners are still actively touching these files; converting the field type would conflict with their in-flight work. Tracked as a deliberate residual; not a public-API hygiene violation per §3a.
- Per-item `///` rustdoc on the deeper `models/a2a/*` and `models/web/*` DTO families was not exhaustively added — these are largely transparent serde DTOs that share their semantics with the upstream A2A protocol spec referenced in `lib.rs`. The crate-level `missing_docs` lint is not enabled in workspace lints.
- `AgentError::Other(#[from] anyhow::Error)` retained as documented composition shim for upstream `anyhow::Result` returns from `systemprompt_database`, `systemprompt_config`, and `systemprompt_models::Config::get`.

---

## Verification

- `cargo check -p systemprompt-agent` :white_check_mark:
- `cargo check --workspace` :white_check_mark:
- `cargo fmt -p systemprompt-agent` :white_check_mark:
- `cargo clippy -p systemprompt-agent --no-deps` — clean
- File-size scan (`find ... | awk '$1>300'`) — empty
- Public-signature anyhow scan (`grep -E '^\s*pub.*anyhow'` over signatures) — empty
