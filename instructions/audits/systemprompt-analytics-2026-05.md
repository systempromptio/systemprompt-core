# systemprompt-analytics Tech Debt Audit

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
| Files >300 lines | 3 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 34 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 3 |
| `anyhow::` references | 37 |
| `async_trait` references | 6 |

**Total scored violations:** 46

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
| All files <=300 lines | FAIL (3) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (34) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (3) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 66 |
| Files over 300 lines | 3 |
| Largest file | `   401 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/behavioral_detector/checks.rs` |

### Files over 300 lines

```
   345 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/queries.rs
   308 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs
   401 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/behavioral_detector/checks.rs
```

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs:54:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs:59:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs:65:                    .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs:72:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs:77:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/services/extractor.rs:92:            .and_then(|v| v.to_str().ok())
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/events.rs:35:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/events.rs:92:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:13:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:51:            sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:79:        let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:93:        let result = sqlx::query!(r#"DELETE FROM funnels WHERE id = $1"#, id.as_str())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:121:                sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:146:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/mutations.rs:182:        let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/funnel/stats.rs:75:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/behavioral.rs:11:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/behavioral.rs:32:    let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/behavioral.rs:61:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:10:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:26:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:43:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:55:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:67:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:79:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:95:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:106:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:118:    let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:138:    let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:152:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:202:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/mutations.rs:227:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/session/queries.rs:331:    let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/conversations.rs:100:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/fingerprint/mutations.rs:74:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/fingerprint/mutations.rs:103:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/fingerprint/mutations.rs:129:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/fingerprint/mutations.rs:146:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/fingerprint/mutations.rs:162:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/engagement.rs:32:        sqlx::query!(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/models/mod.rs:7:#[allow(unused_imports)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/models/cli/request.rs:52:#[allow(clippy::struct_field_names)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/analytics/src/repository/tools/list_queries.rs:35:    #[allow(clippy::too_many_arguments)]
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 6 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 3 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 34 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 3 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
