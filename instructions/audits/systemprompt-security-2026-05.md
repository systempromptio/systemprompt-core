# systemprompt-security Tech Debt Audit

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
| `let _ =` discards | 0 |
| `.ok()` discards | 2 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 3 |
| `async_trait` references | 0 |

**Total scored violations:** 2

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
| No `let _ =` patterns | PASS |
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
| Total .rs files | 14 |
| Files over 300 lines | 0 |
| Largest file | `  205 /var/www/html/systemprompt-core/crates/infra/security/src/extraction/token.rs` |

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/infra/security/src/extraction/header.rs:51:                    .ok()
/var/www/html/systemprompt-core/crates/infra/security/src/auth/validation.rs:83:                    .ok()
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 2 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
