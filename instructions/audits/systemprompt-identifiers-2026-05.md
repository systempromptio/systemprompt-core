# systemprompt-identifiers Tech Debt Audit

**Layer:** shared
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
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 7 |
| `anyhow::` references | 1 |
| `async_trait` references | 0 |

**Total scored violations:** 8

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
| No `let _ =` patterns | PASS |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (1) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (7) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 38 |
| Files over 300 lines | 1 |
| Largest file | `  386 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/macros.rs` |

### Files over 300 lines

```
  386 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/macros.rs
```

---

## Offending Locations

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/url.rs:103:    #[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/path.rs:52:    #[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/profile.rs:39:    #[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/agent.rs:26:    #[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/macros.rs:51:            #[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/macros.rs:80:            #[allow(clippy::expect_used)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/identifiers/src/email.rs:90:    #[allow(clippy::expect_used)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W2)** Remove 7 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
