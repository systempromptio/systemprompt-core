# systemprompt-content Tech Debt Audit

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
| `let _ =` discards | 1 |
| `.ok()` discards | 1 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 8 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 9 |
| `async_trait` references | 10 |

**Total scored violations:** 10

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
| No `let _ =` patterns | FAIL (1) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (8) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 37 |
| Files over 300 lines | 0 |
| Largest file | `   293 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/services/ingestion/mod.rs` |

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/services/ingestion/scanner.rs:73:    let _ = parse_frontmatter(&markdown_text)?;
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/list_items_renderer.rs:94:        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/content/mutations.rs:145:    sqlx::query!("DELETE FROM markdown_content WHERE id = $1", id.as_str())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/content/mutations.rs:155:    let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/link/analytics.rs:60:        let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/link/analytics.rs:77:            sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/link/analytics.rs:85:            sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/link/analytics.rs:128:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/link/analytics.rs:178:        sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/domain/content/src/repository/link/mod.rs:200:        let result = sqlx::query!("DELETE FROM campaign_links WHERE id = $1", id.as_str())
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 1 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 1 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Convert 8 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
