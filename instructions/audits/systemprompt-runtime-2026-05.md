# systemprompt-runtime Tech Debt Audit

**Layer:** app
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
| `#[allow(...)]` | 5 |
| `anyhow::` references | 5 |
| `async_trait` references | 0 |

**Total scored violations:** 6

---

## Architectural Compliance

Layer: `app`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

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
| No `#[allow(...)]` attributes | FAIL (5) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 15 |
| Files over 300 lines | 1 |
| Largest file | `  326 /var/www/html/systemprompt-core/crates/app/runtime/src/context.rs` |

### Files over 300 lines

```
  326 /var/www/html/systemprompt-core/crates/app/runtime/src/context.rs
```

---

## Offending Locations

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/crates/app/runtime/src/registry.rs:86:    #[allow(private_interfaces)]
/var/www/html/systemprompt-core/crates/app/runtime/src/context.rs:196:    #[allow(trivial_casts)]
/var/www/html/systemprompt-core/crates/app/runtime/src/context.rs:256:#[allow(trivial_casts)]
/var/www/html/systemprompt-core/crates/app/runtime/src/context.rs:281:#[allow(trivial_casts)]
/var/www/html/systemprompt-core/crates/app/runtime/src/builder.rs:113:#[allow(trivial_casts)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W2)** Remove 5 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
