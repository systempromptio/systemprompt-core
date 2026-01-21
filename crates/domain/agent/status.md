# systemprompt-agent Compliance

**Layer:** Domain
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ❌ |
| Code Quality | ❌ |

---

## Violations

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/repository/content/artifact.rs` | 565 lines (limit: 300) | Code Quality |
| `src/services/a2a_server/processing/message/stream_processor.rs` | 440 lines | Code Quality |
| `src/repository/task/mod.rs` | 429 lines | Code Quality |
| `src/services/a2a_server/processing/strategies/planned.rs` | 425 lines | Code Quality |
| `src/services/registry.rs` | 408 lines | Code Quality |
| `src/models/a2a/protocol.rs` | 399 lines | Code Quality |
| `src/services/agent_orchestration/lifecycle.rs` | 382 lines | Code Quality |
| `src/services/a2a_server/processing/task_builder.rs` | 380 lines | Code Quality |
| `src/services/external_integrations/webhook/service.rs` | 375 lines | Code Quality |
| `src/services/a2a_server/handlers/request/mod.rs` | 336 lines | Code Quality |
| `src/repository/context/mod.rs` | 328 lines | Code Quality |
| `src/repository/task/constructor/batch.rs` | 315 lines | Code Quality |
| `src/api/routes/contexts/notifications/mod.rs` | 301 lines | Code Quality |
| `src/repository/content/artifact.rs:47` | Inline comment (`// Build metadata...`) | Forbidden Construct |
| `src/repository/content/artifact.rs:55` | Inline comment (`// Insert artifact...`) | Forbidden Construct |
| `src/repository/content/artifact.rs:96` | Inline comment (`// Delete existing...`) | Forbidden Construct |
| `src/repository/content/artifact.rs:106` | Inline comment (`// Insert parts`) | Forbidden Construct |
| `src/repository/content/artifact.rs:339` | Inline comment (`// Parts are deleted...`) | Forbidden Construct |
| `src/repository/content/artifact.rs:352` | Doc comment (`///`) | Forbidden Construct |
| `src/repository/content/artifact.rs:389` | Doc comment (`///`) | Forbidden Construct |
| `src/repository/content/artifact.rs:428` | Doc comment (`///`) | Forbidden Construct |
| `src/repository/content/artifact.rs:508` | Doc comment (`///`) | Forbidden Construct |
| `src/models/a2a/mod.rs:5` | Inline comment (`// Re-export...`) | Forbidden Construct |
| `src/models/a2a/service_status.rs:6` | Doc comment (`///`) | Forbidden Construct |
| `src/services/external_integrations/mcp/mod.rs:7` | Inline comment (`// Re-export...`) | Forbidden Construct |
| `src/services/external_integrations/mcp/mod.rs:13` | Inline comment (`// Re-export...`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:20` | Doc comment (`///`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:23` | Doc comment (`///`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:25` | Doc comment (`///`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:27` | Doc comment (`///`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:29` | Doc comment (`///`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:165` | Doc comment (`///`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:171` | Inline comment (`// Perform startup...`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/mod.rs:174` | Inline comment (`// Get final counts...`) | Forbidden Construct |
| `src/services/agent_orchestration/orchestrator/status.rs:7` | Doc comment (`///`) | Forbidden Construct |
| `src/services/a2a_server/streaming/agent_loader.rs:53` | Doc comment (`///`) | Forbidden Construct |
| `src/services/a2a_server/streaming/handlers/completion.rs:249` | Inline comment (`// Use the new method...`) | Forbidden Construct |
| `src/services/a2a_server/processing/strategies/planned.rs:354-356` | Inline comments | Forbidden Construct |
| `src/services/a2a_server/processing/strategies/planned.rs:385-395` | Inline comments | Forbidden Construct |
| `src/repository/task/mutations.rs:137` | Doc comment (`///`) | Forbidden Construct |
| `src/repository/task/mod.rs:175` | Doc comment (`///`) | Forbidden Construct |
| `src/services/registry.rs:133` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/registry.rs:134` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/content/skill.rs:193` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/content/artifact.rs:357` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/content/artifact.rs:372` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/a2a_server/processing/strategies/plan_executor.rs:134-135` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/context/message/queries.rs:52` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/queries.rs:177` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/a2a_server/streaming/event_loop.rs:151` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/a2a_server/streaming/event_loop.rs:163` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/batch.rs:179` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/batch.rs:234` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/batch.rs:283` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/batch.rs:291` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/single.rs:225` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/converters.rs:14` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/converters.rs:49` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/converters.rs:61` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/converters.rs:73` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/task/constructor/converters.rs:96` | `unwrap_or_default()` | Anti-Pattern |
| `src/models/skill.rs:67` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/mcp/artifact_transformer/mod.rs:55` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/mcp/artifact_transformer/mod.rs:87` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/skills/ingestion.rs:100` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/external_integrations/webhook/service.rs:135` | `unwrap_or_default()` | Anti-Pattern |
| `module.yaml` | Missing required file | Required Structure |
| `src/services/a2a_server/auth/types.rs` | Import ordering | cargo fmt |
| `src/services/a2a_server/auth/validation.rs` | Import ordering | cargo fmt |
| `src/services/a2a_server/standalone.rs` | Import ordering | cargo fmt |
| `src/services/agent_orchestration/process.rs` | Import ordering | cargo fmt |

---

## Commands Run

```
cargo clippy -p systemprompt-agent -- -D warnings  # PASS
cargo fmt -p systemprompt-agent -- --check          # FAIL
```

---

## Actions Required

### Critical (Must Fix)

1. **Run cargo fmt** to fix import ordering in 4 files
2. **Create module.yaml** at crate root with required fields

### High Priority

3. **Remove all inline comments (`//`)** - 20+ occurrences
4. **Remove all doc comments (`///`)** - 15+ occurrences
5. **Replace `unwrap_or_default()` with explicit error handling** - 25 occurrences

### Medium Priority

6. **Split large files** (13 files exceed 300 line limit):
   - `artifact.rs` (565) → split into `artifact_queries.rs`, `artifact_parts.rs`
   - `stream_processor.rs` (440) → split by responsibility
   - `task/mod.rs` (429) → move methods to separate files
   - `planned.rs` (425) → extract helper functions
   - `registry.rs` (408) → split loading and card generation
   - `protocol.rs` (399) → split by type category
   - `lifecycle.rs` (382) → extract state machine
   - `task_builder.rs` (380) → split builder stages
   - `webhook/service.rs` (375) → extract handlers
   - `request/mod.rs` (336) → split by handler type
   - `context/mod.rs` (328) → extract queries
   - `batch.rs` (315) → extract converters
   - `notifications/mod.rs` (301) → split handlers

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unsafe` blocks | ✅ |
| No `unwrap()` | ✅ |
| No `panic!()` | ✅ |
| No `todo!()` | ✅ |
| No TODO/FIXME comments | ✅ |
| No entry layer imports | ✅ |
| Uses SQLX macros | ✅ |
| Uses typed identifiers | ✅ |
| Uses thiserror | ✅ |
| Repository pattern | ✅ |
| Service layering | ✅ |
| cargo clippy | ✅ |
