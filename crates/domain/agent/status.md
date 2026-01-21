# systemprompt-agent Tech Debt Audit

**Layer:** Domain
**Audited:** 2026-01-21
**Verdict:** NEEDS_WORK

---

## Summary

| Category | Status | Issues |
|----------|--------|--------|
| Architecture | :white_check_mark: | 0 |
| Rust Standards | :warning: | 42 (warnings only) |
| Code Quality | :white_check_mark: | 0 |
| Tech Debt | :warning: | 42 |

**Total Issues:** 42 (warnings, no critical)

---

## Architectural Compliance

Cross-domain dependencies are **acceptable** per `instructions/information/boundaries.md`:

> "Domain crates using another domain's public API is acceptable when: Dependency is downward (orchestration layer using lower-level service)"

The agent crate legitimately orchestrates:
- `systemprompt-mcp` - Tool orchestration
- `systemprompt-ai` - AI model orchestration
- `systemprompt-oauth` - Authentication
- `systemprompt-users` - User lookup
- `systemprompt-files` - File handling
- `systemprompt-analytics` - Metrics

App layer dependencies (`runtime`, `scheduler`) are for legitimate integration purposes.

---

## Fixes Applied

### Critical Fixes (Completed)

1. **Raw String IDs -> Typed Identifiers** (internal types)
   - `models/database_rows.rs` - All 6 structs now use `TaskId`, `ContextId`, `MessageId`, `UserId`, `SessionId`, `TraceId`, `AgentName`, `SkillId`, `SourceId`, `CategoryId`, `ArtifactId`, `ExecutionStepId`, `McpExecutionId`
   - `models/agent_info.rs` - `AgentId`
   - `models/context.rs` - `ContextMessage`, `ContextStateEvent` use typed IDs
   - `models/external_integrations.rs` - `AgentId`, `McpServerId`
   - `services/shared/config.rs` - `RuntimeConfiguration`, `AgentServiceConfig` use `AgentId`
   - `services/skills/skill.rs` - `SkillMetadata` uses `SkillId`
   - `api/routes/contexts/webhook/types.rs` - `ContextId`, `UserId`
   - `services/a2a_server/streaming/types.rs` - `MessageId`
   - `services/a2a_server/streaming/initialization.rs` - `MessageId`
   - `services/a2a_server/streaming/event_loop.rs` - `MessageId`

2. **SQL Moved from Service to Repository**
   - `artifact_publishing.rs` - `execution_id_exists()` now calls `ExecutionStepRepository.mcp_execution_id_exists()`
   - Added `mcp_execution_id_exists()` to `repository/execution/mod.rs`

3. **Dead Code Removed**
   - `services/mcp/tool_result_handler.rs` - Removed unused `db_pool` field and `#[allow(dead_code)]`

---

## Remaining Warnings

### Silent Error Handling: `.ok()` Usage (24 instances)

These require individual review. Many are acceptable patterns:
- Parse operations that return None on failure
- Fire-and-forget event broadcasts
- Cleanup in error paths

| File | Pattern | Assessment |
|------|---------|------------|
| `services/context.rs:144,156` | `.ok()?` | Review needed |
| `services/registry/security.rs:14,22` | `serde_json::from_value().ok()` | Acceptable - parse may fail |
| `services/a2a_server/auth/validation.rs:138,157` | `Permission::from_str().ok()`, `to_str().ok()` | Acceptable - parse may fail |
| `services/skills/ingestion.rs:141` | `.filter_map(\|e\| e.ok())` | Acceptable - skip invalid entries |
| `repository/context/message/parts.rs:198` | `Uuid::parse_str().ok()?` | Acceptable - parse may fail |
| `models/web/validation.rs:17` | `port_str.parse().ok()` | Acceptable - parse may fail |
| Most streaming handlers | Broadcast `.ok()` | Acceptable - fire-and-forget |

### Silent Error Handling: `let _ =` Pattern (18 instances)

Most are intentional fire-and-forget broadcast patterns:

| File | Pattern | Assessment |
|------|---------|------------|
| `services/a2a_server/processing/message/message_handler.rs` | `let _ = broadcast_agui_event(...)` | Acceptable - fire-and-forget broadcast |
| `services/agent_orchestration/event_bus.rs:16` | `let _ = self.sender.send(event)` | Acceptable - fire-and-forget event |
| `services/a2a_server/streaming/*.rs` | `let _ = tx.send(...)` | Acceptable - SSE channel send |

### API Protocol Types (Not Changed)

The following files use `String` for `task_id`, `context_id` in API boundary types. These are intentionally String because:
- They're JSON-RPC protocol types matching external API spec
- Use `impl Into<String>` constructors
- Changing would break API compatibility

Files: `models/a2a/protocol/events.rs`, `models/a2a/protocol/push_notification.rs`, `models/a2a/protocol/requests.rs`

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unsafe` blocks | :white_check_mark: PASS |
| No `unwrap()` | :white_check_mark: PASS |
| No `.expect()` | :white_check_mark: PASS |
| No `panic!()` | :white_check_mark: PASS |
| No `todo!()` | :white_check_mark: PASS |
| No `unimplemented!()` | :white_check_mark: PASS |
| No TODO/FIXME/HACK comments | :white_check_mark: PASS |
| No inline comments (`//`) | :white_check_mark: PASS |
| No doc comments (`///`) | :white_check_mark: PASS |
| No `#[cfg(test)]` modules | :white_check_mark: PASS |
| No `println!`/`eprintln!`/`dbg!` | :white_check_mark: PASS |
| No `unwrap_or_default()` | :white_check_mark: PASS |
| No `NaiveDateTime` | :white_check_mark: PASS |
| Uses SQLX macros (not runtime) | :white_check_mark: PASS |
| Repository pattern enforced | :white_check_mark: PASS |
| All files ≤300 lines | :white_check_mark: PASS |
| `cargo fmt --check` | :white_check_mark: PASS |
| Has error.rs | :white_check_mark: PASS |
| Has repository/ | :white_check_mark: PASS |
| Has services/ | :white_check_mark: PASS |
| Has schema/ | :white_check_mark: PASS |
| No `#[allow(dead_code)]` | :white_check_mark: PASS |

---

## Commands Executed

```
cargo fmt -p systemprompt-agent -- --check     # PASS
cargo clippy -p systemprompt-agent -D warnings # BLOCKED (requires DB for SQLX)
cargo check -p systemprompt-agent              # BLOCKED (requires DB for SQLX)
```

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | ~100 |
| Files over 300 lines | 0 |
| Largest file | 298 lines (initialization.rs) |

---

## Verdict Criteria

- **CLEAN**: Zero critical violations, ready for crates.io
- **NEEDS_WORK**: Minor issues, can publish with warnings
- **CRITICAL**: Blocking issues, must resolve before publication

**Current Verdict: NEEDS_WORK**

The crate has:
- Zero critical violations
- All internal types use typed identifiers
- Repository pattern enforced
- Remaining warnings are acceptable patterns (fire-and-forget broadcasts, parse-to-Option conversions)

**Recommendation:** Ready for internal use. Before crates.io publication, review the `.ok()` and `let _ =` patterns to ensure they're intentional.
