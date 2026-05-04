# systemprompt-logging Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04
**Verdict:** CRITICAL

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 23 |
| `.ok()` discards | 3 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 2 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 30 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 37 |
| `async_trait` references | 2 |

**Total scored violations:** 58

---

## Architectural Compliance

Layer: `infra`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | FAIL (23) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (2) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (30) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 51 |
| Files over 300 lines | 2 |
| Largest file | `   392 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/models.rs` |

### Files over 300 lines

```
   392 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/models.rs
   333 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/layer/proxy.rs:44:            let _ = writeln!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/layer/mod.rs:79:            let _ = writeln!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/layer/mod.rs:134:        let _ = self.sender.send(LogCommand::Entry(Box::new(entry)));
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/layer/mod.rs:136:            let _ = self.sender.send(LogCommand::FlushNow);
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/display.rs:18:    let _ = writeln!(stdout, "{args}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:72:        let _ = write!(stdout, "\x1B[2J\x1B[1;1H");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:77:        let _ = writeln!(stdout, "{content}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:84:                let _ = writeln!(stdout, "{json}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:94:                let _ = writeln!(stdout, "{json}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:104:                let _ = write!(stdout, "{yaml}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:112:        let _ = writeln!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:122:        let _ = writeln!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:320:        let _ = writeln!(stdout, "{}", Theme::color(&banner, EmphasisType::Dim));
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/service.rs:331:        let _ = writeln!(stdout, "{}", Theme::color(&banner, EmphasisType::Dim));
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/module.rs:10:    let _ = writeln!(out, "{args}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/summary.rs:139:            let _ = writeln!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/summary.rs:219:            let _ = writeln!(stdout, "  \u{2022} {colored}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/prompts.rs:85:                let _ = writeln!(stdout);
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/prompts.rs:154:                let _ = writeln!(stdout);
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/table.rs:8:    let _ = write!(out, "{args}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/table.rs:13:    let _ = writeln!(out, "{args}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/services/cli/startup.rs:7:    let _ = writeln!(out, "{args}");
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/mod.rs:53:            let _ = writeln!(stdout, "{entry}");
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/queries.rs:36:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/models/log_row.rs:125:                    .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/log_lookup_queries.rs:37:                .ok()
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/mutations.rs:27:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/mutations.rs:64:    let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/mutations.rs:86:    let result = sqlx::query!("DELETE FROM logs WHERE id = $1", id_str)
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/mutations.rs:101:    let result = sqlx::query!("DELETE FROM logs WHERE id = ANY($1)", &id_strs)
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/mutations.rs:110:    let result = sqlx::query!("DELETE FROM logs")
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/operations/mutations.rs:122:    let result = sqlx::query!("DELETE FROM logs WHERE timestamp < $1", cutoff)
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/repository/analytics/mod.rs:31:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/layer/mod.rs:106:            sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:15:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:26:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:49:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:64:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:82:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:112:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:138:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:155:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/ai_trace_queries.rs:179:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/step_queries.rs:14:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/step_queries.rs:37:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/step_queries.rs:107:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/step_queries.rs:121:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/step_queries.rs:149:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/queries.rs:16:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/queries.rs:55:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/queries.rs:86:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/mcp_trace_queries.rs:14:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/mcp_trace_queries.rs:45:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/mcp_trace_queries.rs:75:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/trace/mcp_trace_queries.rs:109:    let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/logging/src/lib.rs:43:    "sqlx::query=warn",
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 23 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 3 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 2 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 30 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
