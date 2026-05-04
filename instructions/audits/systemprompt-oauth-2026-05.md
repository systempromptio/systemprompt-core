# systemprompt-oauth Tech Debt Audit

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
| `let _ =` discards | 2 |
| `.ok()` discards | 15 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 47 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 71 |
| `async_trait` references | 4 |

**Total scored violations:** 66

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
| No `let _ =` patterns | FAIL (2) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (1) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (47) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 62 |
| Files over 300 lines | 1 |
| Largest file | `   324 /var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/mod.rs` |

### Files over 300 lines

```
   324 /var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/mod.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/cowork.rs:144:        let _ = write!(out, "{byte:02x}");
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/cowork.rs:155:        let _ = write!(out, "{byte:02x}");
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/providers.rs:79:            .filter_map(|p| p.parse().ok())
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/providers.rs:85:            .filter_map(|a| a.parse().ok())
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:24:            .ok()?;
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:36:            .ok()
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:47:            .ok()
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:59:            .ok()?;
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:69:            .ok()?;
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:104:            .ok()?
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:109:            .ok()??;
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:120:            .ok()?;
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/session/lookup.rs:130:            .ok()?;
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/http.rs:12:                .ok()
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/user.rs:60:                    .ok()
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/auth_code.rs:201:                .ok()
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/jwt/authorization.rs:114:                    .ok()
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/exchange_code.rs:19:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/exchange_code.rs:34:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/setup_token.rs:71:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/setup_token.rs:89:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/setup_token.rs:124:        let rows_affected = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/setup_token.rs:140:        let rows_affected = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/setup_token.rs:156:        let rows_affected = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/webauthn.rs:115:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/webauthn.rs:141:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/webauthn.rs:180:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/refresh_token.rs:75:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/refresh_token.rs:101:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/refresh_token.rs:126:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/refresh_token.rs:138:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/refresh_token.rs:151:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/user.rs:18:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/user.rs:38:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/mutations.rs:32:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/mutations.rs:111:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/mutations.rs:155:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/mutations.rs:174:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/mutations.rs:186:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/mutations.rs:199:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/relations.rs:61:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/relations.rs:79:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/relations.rs:96:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/relations.rs:116:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/relations.rs:133:        let rows = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:23:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:29:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:35:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:41:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:47:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:79:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:106:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:129:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:150:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/inserts.rs:174:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/auth_code.rs:97:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/auth_code.rs:144:        let row = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/oauth/auth_code.rs:231:        sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/cleanup.rs:8:        let result = sqlx::query!("DELETE FROM oauth_clients WHERE is_active = false")
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/cleanup.rs:16:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/cleanup.rs:29:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/cleanup.rs:43:        let result = sqlx::query!(
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/cleanup.rs:54:        let result = sqlx::query!("DELETE FROM oauth_clients WHERE last_used_at < $1", cutoff)
/var/www/html/systemprompt-core/crates/domain/oauth/src/repository/client/cleanup.rs:118:        sqlx::query!(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/crates/domain/oauth/src/services/generation.rs:204:#[allow(clippy::too_many_arguments)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 2 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 15 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 47 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
