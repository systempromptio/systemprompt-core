# systemprompt-ai Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Verdict:** CRITICAL

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 6 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 10 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 48 |
| `async_trait` references | 18 |

**Total scored violations:** 17

---

## Architectural Compliance

Layer: `domain`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (1) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (10) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 102 |
| Files over 300 lines | 1 |
| Largest file | `   325 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/models/providers/anthropic.rs` |

### Files over 300 lines

```
   325 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/models/providers/anthropic.rs
```

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/services/tools/adapter.rs:27:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/services/tools/adapter.rs:50:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/services/tools/adapter.rs:101:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/services/tools/adapter.rs:160:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/services/providers/gemini/streaming.rs:85:        .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/services/core/request_storage/async_operations.rs:19:        .ok()
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_safety_findings.rs:36:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_quota_buckets.rs:57:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_request_payloads.rs:40:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_request_payloads.rs:70:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_gateway_policies.rs:39:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_gateway_policies.rs:72:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_requests/mutations.rs:92:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_requests/mutations.rs:124:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_requests/message_operations.rs:136:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/ai/src/repository/ai_requests/message_operations.rs:159:        let result = sqlx::query!(
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 6 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 10 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
