# systemprompt-cloud Tech Debt Audit

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
| `.ok()` discards | 5 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 2 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 18 |
| `async_trait` references | 0 |

**Total scored violations:** 9

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
| All files <=300 lines | FAIL (2) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 27 |
| Files over 300 lines | 2 |
| Largest file | `   327 /var/www/html/systemprompt-core/crates/infra/cloud/src/tenants.rs` |

### Files over 300 lines

```
   304 /var/www/html/systemprompt-core/crates/infra/cloud/src/api_client/client.rs
   327 /var/www/html/systemprompt-core/crates/infra/cloud/src/tenants.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/crates/infra/cloud/src/credentials_bootstrap.rs:146:        let _ = CREDENTIALS.set(None);
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/infra/cloud/src/credentials_bootstrap.rs:110:            .ok()
/var/www/html/systemprompt-core/crates/infra/cloud/src/credentials_bootstrap.rs:114:            .ok()
/var/www/html/systemprompt-core/crates/infra/cloud/src/credentials_bootstrap.rs:122:                .ok()
/var/www/html/systemprompt-core/crates/infra/cloud/src/cli_session/store.rs:165:            .ok()?;
/var/www/html/systemprompt-core/crates/infra/cloud/src/cli_session/store.rs:168:            .ok()
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/crates/infra/cloud/src/checkout/client.rs:42:#[allow(clippy::struct_field_names)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 1 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 5 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 2 files exceeding 300 lines into focused submodules.
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**NEEDS_WORK**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
