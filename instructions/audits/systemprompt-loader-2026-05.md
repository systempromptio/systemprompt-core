# systemprompt-loader Tech Debt Audit

**Layer:** infra
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
| `.ok()` discards | 2 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 17 |
| `async_trait` references | 0 |

**Total scored violations:** 4

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
| No `let _ =` patterns | FAIL (1) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (1) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 8 |
| Files over 300 lines | 1 |
| Largest file | `  426 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/loader/src/config_loader.rs` |

### Files over 300 lines

```
  426 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/loader/src/config_loader.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/loader/src/config_loader.rs:119:        let _ = Self::load_from_path(path)?;
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/loader/src/extension_loader.rs:138:                    .ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/infra/loader/src/extension_loader.rs:139:                let debug_mtime = fs::metadata(&debug_binary).and_then(|m| m.modified()).ok();
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 1 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 2 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
