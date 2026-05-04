# systemprompt-sync Tech Debt Audit

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
| `.ok()` discards | 1 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 3 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 14 |
| `async_trait` references | 2 |

**Total scored violations:** 5

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
| No raw `sqlx::query` outside allowlist | FAIL (3) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 24 |
| Files over 300 lines | 1 |
| Largest file | `  302 /var/www/html/systemprompt-core/crates/app/sync/src/api_client.rs` |

### Files over 300 lines

```
  302 /var/www/html/systemprompt-core/crates/app/sync/src/api_client.rs
```

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/app/sync/src/diff/content.rs:112:                .ok()
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/crates/app/sync/src/database/upsert.rs:26:    let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/app/sync/src/database/upsert.rs:69:    let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/app/sync/src/database/upsert.rs:150:    let result = sqlx::query!(
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 1 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 3 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
