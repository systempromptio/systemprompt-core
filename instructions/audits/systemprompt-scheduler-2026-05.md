# systemprompt-scheduler Tech Debt Audit

**Layer:** app
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
| `.ok()` discards | 2 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 2 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 8 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 39 |
| `async_trait` references | 16 |

**Total scored violations:** 12

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
| All files <=300 lines | FAIL (2) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (8) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 25 |
| Files over 300 lines | 2 |
| Largest file | `  332 /var/www/html/systemprompt-core/crates/app/scheduler/src/services/orchestration/process_cleanup.rs` |

### Files over 300 lines

```
  327 /var/www/html/systemprompt-core/crates/app/scheduler/src/services/scheduling/mod.rs
  332 /var/www/html/systemprompt-core/crates/app/scheduler/src/services/orchestration/process_cleanup.rs
```

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/app/scheduler/src/services/orchestration/process_cleanup.rs:38:                .and_then(|pid| pid.trim().parse::<u32>().ok())
/var/www/html/systemprompt-core/crates/app/scheduler/src/services/orchestration/process_cleanup.rs:271:            .ok()?;
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/jobs/mod.rs:30:        sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/jobs/mod.rs:94:        sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/jobs/mod.rs:118:        sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/analytics/mod.rs:17:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/security/mod.rs:25:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/security/mod.rs:53:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/security/mod.rs:81:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/app/scheduler/src/repository/security/mod.rs:106:        let rows = sqlx::query!(
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 2 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 2 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 8 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
