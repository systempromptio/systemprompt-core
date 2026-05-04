# systemprompt-security Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04
**Re-validated (Wave B1):** 2026-05-04
**Verdict:** CLEAN

---

## Summary

| Category | Baseline | Wave B1 |
|----------|----------|---------|
| unwrap()/expect() | 0 | 0 |
| panic!()/todo!()/unimplemented!() | 0 | 0 |
| println!/eprintln!/dbg! | 0 | 0 |
| `let _ =` discards | 0 | 0 |
| `.ok()` discards (with logging) | 2 | 2 (each preceded by `tracing::debug!`, retained as fall-through) |
| Inline `//` WHAT-comments | 0 | 0 |
| Doc `///` coverage on pub items | 0 / 73 | 73 / 73 |
| Files >300 lines | 0 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |
| `anyhow::` references in public API | 3 | 0 |
| `async_trait` references | 0 | 0 |

**Total scored violations:** 0

---

## Wave B1 Fixes Applied

- Added crate-level `//!` with feature-flag matrix and runnable example.
- Added `//!` module docs to every `pub mod` (`auth`, `error`, `extraction`,
  `jwt`, `manifest_signing`, `services`, `session`).
- Added `///` doc comments to every `pub` item across all modules
  (functions, structs, enums, struct fields, type aliases, constants).
- Introduced `error.rs` exposing thiserror-derived `AuthError`,
  `JwtError`, `ManifestSigningError` and matching `*Result` aliases.
- Replaced all `anyhow::Result` / `anyhow!` in public signatures:
  - `AuthValidationService::validate_request` — `AuthResult<RequestContext>`.
  - `JwtService::generate_admin_token` — `JwtResult<JwtToken>`.
  - `SessionGenerator::generate` — `JwtResult<SessionToken>`.
  - `manifest_signing::*` — `ManifestSigningResult<_>` (was `Result<_, String>`).
- Dropped `anyhow` from `Cargo.toml`.
- Documented the rationale for the `signing_key()` `OnceLock` race
  fall-back via a Why comment.
- Adjusted downstream caller (`crates/entry/api/src/routes/gateway/cowork.rs`)
  to render `ManifestSigningError` via `Display` rather than direct serde
  (the previous code relied on `Serialize` for `String`).
- Added `[package.metadata.docs.rs] all-features = true` to `Cargo.toml`.

The two surviving `.ok()` calls (one in `auth/validation.rs`, one in
`extraction/header.rs`) are preceded by `tracing::debug!` invocations
inside the `map_err` closure and convert non-ASCII header errors into
"absent" — which is the documented behaviour. Replacing them with
propagation would change semantics.

---

## Architectural Compliance

Layer: `infra`. Per `instructions/information/boundaries.md` dependencies
flow downward only; this crate only imports from `shared/*` and
`infra/config`.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No bare `let _ =` patterns | PASS |
| All pub items carry `///` rustdoc | PASS |
| All `pub mod` carry `//!` rustdoc | PASS |
| All files <=300 lines | PASS (largest now `extraction/token.rs` ~280) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |
| No `anyhow::` in public signatures | PASS |
| `cargo fmt -p systemprompt-security --check` | PASS |
| `cargo build -p systemprompt-security --all-features` | PASS |
| `cargo clippy -p systemprompt-security --lib --all-features --no-deps -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-security --no-deps --all-features` | PASS |
| `just check-bans-crate systemprompt-security` | PASS |

`--all-targets` clippy is currently unable to run cleanly because
`systemprompt-logging::services::cli` (other slice) violates the new
`let_underscore_must_use` workspace lint introduced in commit `87d901a0`;
that crate is owned by Wave B's logging slice, not this slice.

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 15 (was 14 — added `error.rs`) |
| Files over 300 lines | 0 |
| Largest file | `extraction/token.rs` (~280 lines) |

---

## Verdict

**CLEAN**
