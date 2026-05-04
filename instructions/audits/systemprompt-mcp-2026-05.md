# systemprompt-mcp Tech Debt Audit

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
| `let _ =` discards | 4 |
| `.ok()` discards | 27 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 4 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 18 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 2 |
| `anyhow::` references | 110 |
| `async_trait` references | 40 |

**Total scored violations:** 55

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
| No `let _ =` patterns | FAIL (4) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (4) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (18) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (2) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 94 |
| Files over 300 lines | 4 |
| Largest file | `   348 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs` |

### Files over 300 lines

```
   314 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/rbac.rs
   318 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/ui_renderer/templates/form.rs
   316 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/orchestrator/mod.rs
   348 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/ui_renderer/templates/form.rs:148:                    let _ = write!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/ui_renderer/templates/dashboard/section.rs:164:            let _ = write!(acc, "<th>{}</th>", html_escape(c));
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/ui_renderer/templates/dashboard/section.rs:186:                        let _ = write!(acc, "<td>{}</td>", html_escape(c.trim_matches('"')));
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/ui_renderer/templates/dashboard/mod.rs:99:                    let _ = write!(
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/orchestration/loader.rs:153:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:96:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:136:            .and_then(|v| serde_json::to_string(v).ok());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/lib.rs:91:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/lib.rs:97:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/lib.rs:102:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/tool.rs:142:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/rbac.rs:153:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/rbac.rs:163:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/rbac.rs:174:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/rbac.rs:175:        .and_then(|s| systemprompt_models::auth::parse_permissions(s).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/rbac.rs:194:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/session_manager.rs:69:        let repository = McpSessionRepository::new(db_pool).ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/middleware/mod.rs:25:        .and_then(|v| v.to_str().ok());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/database/state.rs:13:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/database/state.rs:14:        .and_then(|m| m.modified().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/database/state.rs:15:        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/database/state.rs:23:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs:117:        .and_then(|line| line.trim().parse::<u32>().ok()))
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs:157:        .filter_map(|line| line.trim().parse::<u32>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs:200:        .filter_map(|entry| entry.metadata().ok().map(|m| m.ino()))
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs:265:                .and_then(|port_part| port_part.parse::<u16>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs:302:        .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/pid_manager.rs:317:        .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/tool_provider/conversions.rs:15:            .and_then(|c| serde_json::to_value(c).ok()),
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/tool_provider/conversions.rs:46:            .and_then(|m| serde_json::to_value(m).ok()),
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/client/http_client_with_context.rs:200:            .and_then(|v| v.to_str().ok())
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:52:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:99:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:138:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:171:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:251:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/tool_usage/mod.rs:290:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:43:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:71:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:109:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:125:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:141:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:156:        let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/session/mod.rs:171:        let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/artifact/mod.rs:56:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/artifact/mod.rs:86:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/artifact/mod.rs:131:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/artifact/mod.rs:178:        let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/repository/artifact/mod.rs:189:        let result = sqlx::query!(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/process/cleanup.rs:47:#[allow(clippy::unnecessary_wraps)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/mcp/src/services/orchestrator/mod.rs:41:    #[allow(clippy::needless_pass_by_value)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 4 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 27 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 4 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 18 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 2 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
