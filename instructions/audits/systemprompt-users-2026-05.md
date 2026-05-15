# systemprompt-users Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave C1)
**Verdict:** CLEAN

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
| Doc `///` comments | added on every `pub` item touched |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 (all 16 baseline hits are macro forms — false-positive) |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in PUBLIC signatures | 0 (was 9) |
| `async_trait` references | 7 (all on `dyn`-used traits — `Job`, `UserProvider`, `RoleProvider`) |

**Total scored violations:** 0

---

## Wave C1 Fixes Applied

- `error.rs`: added crate-level `//!` doc, `///` on every variant, a new `Pool(String)` variant, a `From<anyhow::Error> for UserError` impl that maps into `Pool`, and a public `UserResult<T>` alias.
- `services/user/mod.rs`, `services/api_key_service.rs`, `services/device_cert_service.rs`: `pub fn new(db: &DbPool) -> anyhow::Result<Self>` → `Result<Self>`.
- `repository/mod.rs`, `repository/banned_ip/{mod,queries,listing}.rs`: `use anyhow::Result;` → `use crate::error::Result;`.
- `jobs/cleanup_anonymous_users.rs`: `anyhow::anyhow!` → `ProviderError::Configuration`.
- `lib.rs`: added `//!` crate-level docs with feature-flag matrix; `error` module promoted to `pub mod`; `UserResult` re-exported.
- `Cargo.toml`: added `[package.metadata.docs.rs] all-features = true`.

## sqlx Verification

`grep -E 'sqlx::query[^_!a-zA-Z]' crates/domain/users/src` → no matches. All 16 baseline hits are `sqlx::query!` / `sqlx::query_as!` / `sqlx::query_scalar!` macros.

---

## Cross-Crate Shim Applied

`crates/app/scheduler/src/jobs/behavioral_analysis.rs`: `BannedIpRepository::new(...)?` now returns `UserError`; added a `.map_err(|e| ProviderError::Configuration(e.to_string()))` shim and adjusted the `log_ban_result` helper signature to `Result<(), UserError>`. The flag-side `log_flag_result` keeps `Result<(), anyhow::Error>` (analytics fingerprint repository still returns `anyhow::Result`).

---

## Passing Checks

| Check | Status |
|-------|--------|
| `cargo build -p systemprompt-users --all-features` | PASS |
| `cargo clippy -p systemprompt-users --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-users --no-deps --all-features` | PASS |
| `cargo build --workspace --all-features` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Manual bans scan on `crates/domain/users/src` | PASS |

`just check-bans-crate systemprompt-users` mis-resolves to `crates/shared/models/src/users` because of the recipe's `find -maxdepth 4` glob — known recipe quirk, the manual scan above covers the same checks.

---

## Verdict

**CLEAN**
