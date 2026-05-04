# systemprompt-traits Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Verdict:** CRITICAL

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 17 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 20 |
| `async_trait` references | 35 |

**Total scored violations:** 17

---

## Architectural Compliance

Layer: `shared`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | FAIL (17) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 29 |
| Files over 300 lines | 0 |
| Largest file | `  237 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/ai_providers.rs` |

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:40:        let _ = self.unbounded_send(StartupEvent::PhaseStarted { phase });
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:44:        let _ = self.unbounded_send(StartupEvent::PhaseCompleted { phase });
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:48:        let _ = self.unbounded_send(StartupEvent::PhaseFailed {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:55:        let _ = self.unbounded_send(StartupEvent::PortAvailable { port });
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:59:        let _ = self.unbounded_send(StartupEvent::PortConflict { port, pid });
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:63:        let _ = self.unbounded_send(StartupEvent::ModulesLoaded { count, modules });
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:67:        let _ = self.unbounded_send(StartupEvent::McpServerStarting {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:74:        let _ = self.unbounded_send(StartupEvent::McpServerHealthCheck {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:82:        let _ = self.unbounded_send(StartupEvent::McpServerReady {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:91:        let _ = self.unbounded_send(StartupEvent::McpServerFailed {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:98:        let _ = self.unbounded_send(StartupEvent::AgentStarting {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:105:        let _ = self.unbounded_send(StartupEvent::AgentReady {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:113:        let _ = self.unbounded_send(StartupEvent::AgentFailed {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:120:        let _ = self.unbounded_send(StartupEvent::ServerListening {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:127:        let _ = self.unbounded_send(StartupEvent::Warning {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:134:        let _ = self.unbounded_send(StartupEvent::Info {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/traits/src/startup_events/ext.rs:145:        let _ = self.unbounded_send(StartupEvent::StartupComplete {
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 17 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
