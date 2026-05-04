# systemprompt-files Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Verdict:** NEEDS_WORK

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 1 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 7 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 7 |
| `async_trait` references | 6 |

**Total scored violations:** 9

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
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (7) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 30 |
| Files over 300 lines | 0 |
| Largest file | `  273 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/services/upload/validator.rs` |

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/jobs/file_ingestion.rs:226:            .ok(),
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/file/mod.rs:196:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/file/mod.rs:216:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/file/stats.rs:23:        let row = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/content/mod.rs:51:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/content/mod.rs:70:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/content/mod.rs:153:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/repository/content/mod.rs:166:        let result = sqlx::query!(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/files/src/config/types.rs:16:#[allow(clippy::struct_excessive_bools)]
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 1 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Convert 7 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
