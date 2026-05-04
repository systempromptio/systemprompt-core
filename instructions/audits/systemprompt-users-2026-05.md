# systemprompt-users Tech Debt Audit

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
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 16 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references | 9 |
| `async_trait` references | 7 |

**Total scored violations:** 16

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
| No raw `sqlx::query` outside allowlist | FAIL (16) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 27 |
| Files over 300 lines | 0 |
| Largest file | `  280 /var/www/html/systemprompt-core/crates/domain/users/src/repository/user/operations.rs` |

---

## Offending Locations

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/crates/domain/users/src/repository/device_cert.rs:74:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/api_key.rs:78:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/api_key.rs:93:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/merge.rs:14:        let sessions_result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/merge.rs:26:        let tasks_result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/merge.rs:38:        sqlx::query!(r#"DELETE FROM users WHERE id = $1"#, source_id.as_str())
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/operations.rs:246:        let result = sqlx::query!(r#"DELETE FROM users WHERE id = $1"#, id.as_str())
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/operations.rs:260:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/session.rs:82:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/session.rs:98:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/list.rs:138:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/user/list.rs:156:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/banned_ip/queries.rs:56:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/banned_ip/queries.rs:92:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/banned_ip/queries.rs:135:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/users/src/repository/banned_ip/queries.rs:149:        let result = sqlx::query!(
```

---

## Recommendations for Wave 1/2

- **(W1)** Convert 16 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
