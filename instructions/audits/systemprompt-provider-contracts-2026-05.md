# systemprompt-provider-contracts Tech Debt Audit

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
| `let _ =` discards | 1 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 15 |
| `async_trait` references | 23 |

**Total scored violations:** 3

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
| No `let _ =` patterns | FAIL (1) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (1) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 18 |
| Files over 300 lines | 1 |
| Largest file | `  316 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/provider-contracts/src/llm.rs` |

### Files over 300 lines

```
  316 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/provider-contracts/src/llm.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/provider-contracts/src/sitemap.rs:44:        let _ = base_url;
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/shared/provider-contracts/src/web_config/mod.rs:52:#[allow(clippy::struct_field_names)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 1 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
