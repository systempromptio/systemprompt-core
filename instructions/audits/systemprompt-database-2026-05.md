# systemprompt-database Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04
**Verdict:** CRITICAL

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
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 29 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 31 |
| `async_trait` references | 9 |

**Total scored violations:** 33

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
| No raw `sqlx::query` outside allowlist | FAIL (29) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 32 |
| Files over 300 lines | 1 |
| Largest file | `  320 /var/www/html/systemprompt-core/crates/infra/database/src/lifecycle/installation.rs` |

### Files over 300 lines

```
  320 /var/www/html/systemprompt-core/crates/infra/database/src/lifecycle/installation.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/crates/infra/database/src/services/display.rs:11:    let _ = writeln!(stdout, "{args}");
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:65:            .ok()
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/crates/infra/database/src/repository/cleanup.rs:19:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/cleanup.rs:32:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/cleanup.rs:46:        let result = sqlx::query!("DELETE FROM logs WHERE timestamp < $1", cutoff)
/var/www/html/systemprompt-core/crates/infra/database/src/repository/cleanup.rs:53:        let result = sqlx::query!("DELETE FROM oauth_refresh_tokens WHERE expires_at < NOW()")
/var/www/html/systemprompt-core/crates/infra/database/src/repository/cleanup.rs:60:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/conversion.rs:98:    mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/conversion.rs:100:) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:83:        let query_obj = sqlx::query(sql);
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:114:        let query_obj = sqlx::query(sql);
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:131:        let query_obj = sqlx::query(sql);
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:148:        let query_obj = sqlx::query(sql);
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:203:        sqlx::query("SELECT 1")
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:213:            sqlx::query(&statement)
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:225:        let rows = sqlx::query(sql)
/var/www/html/systemprompt-core/crates/infra/database/src/services/postgres/mod.rs:242:        let mut query_obj = sqlx::query(sql);
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:44:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:69:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:81:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:110:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:133:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:146:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:158:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:171:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:183:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:211:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:224:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:236:        sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:254:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/infra/database/src/repository/service.rs:283:        let result = sqlx::query!(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/crates/infra/database/src/services/provider.rs:65:#[allow(async_fn_in_trait)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 1 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 1 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 29 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
