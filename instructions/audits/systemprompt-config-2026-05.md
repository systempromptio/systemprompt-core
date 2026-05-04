# systemprompt-config Tech Debt Audit

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
| `let _ =` discards | 0 |
| `.ok()` discards | 18 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 1 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 |
| `anyhow::` references | 35 |
| `async_trait` references | 0 |

**Total scored violations:** 20

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
| All files <=300 lines | FAIL (1) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (1) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 15 |
| Files over 300 lines | 1 |
| Largest file | `  403 /var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs` |

### Files over 300 lines

```
  403 /var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs
```

---

## Offending Locations

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:168:            .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:173:            .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:178:            .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:185:                            .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:195:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:199:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:202:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:205:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:207:            sync_token: std::env::var("SYNC_TOKEN").ok().filter(|s| !s.is_empty()),
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:209:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:212:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:215:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:217:            github: std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty()),
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:219:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:220:                .or_else(|| std::env::var("KIMI_API_KEY").ok())
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:223:                .ok()
/var/www/html/systemprompt-core/crates/infra/config/src/bootstrap/secrets.rs:224:                .or_else(|| std::env::var("DASHSCOPE_API_KEY").ok())
/var/www/html/systemprompt-core/crates/infra/config/src/services/manager.rs:236:                .or_else(|| std::env::var(var_name).ok())
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/crates/infra/config/src/services/schema_validation.rs:37:#[allow(
```

---

## Recommendations for Wave 1/2

- **(W2)** Audit 18 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 1 files exceeding 300 lines into focused submodules.
- **(W2)** Remove 1 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
