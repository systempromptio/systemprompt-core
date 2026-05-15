# systemprompt-events Tech Debt Audit

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
| `.ok()` discards | 0 | 0 |
| Inline `//` comments | 0 | 0 |
| Doc `///` coverage on pub items | 0 / 24 | 24 / 24 |
| Files >300 lines | 0 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |
| `anyhow::` references | 0 | 0 |
| `async_trait` references | 4 | 4 (documented `dyn`-compat carve-out) |

**Total scored violations:** 0

---

## Wave B1 Fixes Applied

- Added crate-level `//!` with feature-flag matrix and runnable example.
- Added `//!` module docs to every `pub mod` (`error`, `services`, `sse`).
- Added `///` doc comments to every `pub` item (trait, methods, structs,
  type aliases, constants, statics, free functions).
- Added `[package.metadata.docs.rs] all-features = true` to `Cargo.toml`.
- Introduced `error.rs` exposing `EventError` / `EventResult` (thiserror,
  composes `serde_json::Error` via `#[from]`); dropped now-unused
  `anyhow` dep.
- Documented the `#[async_trait]` retention rationale on the
  `Broadcaster` trait (must remain `dyn`-compatible because the static
  `EventRouter` and global broadcaster registries store it behind trait
  objects).

---

## Architectural Compliance

Layer: `infra`. Per `instructions/information/boundaries.md` dependencies
flow downward only; this crate sits between `shared` and the rest of the
infra layer and does not violate that flow.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| No inline `//` WHAT-comments | PASS |
| All pub items carry `///` rustdoc | PASS |
| All `pub mod` carry `//!` rustdoc | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |
| No `anyhow::` in public signatures | PASS |
| `cargo fmt -p systemprompt-events --check` | PASS |
| `cargo build -p systemprompt-events --all-features` | PASS |
| `cargo clippy -p systemprompt-events --all-targets --all-features -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-events --no-deps --all-features` | PASS |
| `just check-bans-crate systemprompt-events` | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 6 (was 5 — added `error.rs`) |
| Files over 300 lines | 0 |
| Largest file | `services/broadcaster.rs` (~225 lines after rustdoc) |

---

## Verdict

**CLEAN**
