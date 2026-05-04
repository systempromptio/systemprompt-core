# systemprompt-template-provider Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04
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
| Doc `///` comments on `pub` items | all (was 0) |
| Module `//!` headers | all `pub mod` (was 0) |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in public signatures | 0 |
| Public errors typed via `thiserror` | yes (`TemplateLoaderError`) |
| `[package.metadata.docs.rs] all-features = true` | yes |
| `async_trait` references | 4 — `dyn`-required (see `DynTemplateLoader`) |

**Total scored violations:** 0

---

## Wave A5 Fixes Applied

- `lib.rs`: added `//!` crate header with feature-flag matrix and runnable
  `no_run` example. Added `///` to every `pub type` (six `Dyn*` aliases).
- `traits/mod.rs`: added `//!` module header explaining the split between
  locally-defined loader traits and the cross-crate provider contracts.
- `traits/error.rs`: added `///` on `TemplateLoaderError`, every variant,
  the `Io { path, source }` fields, the `io()` constructor, and the `Result`
  alias.
- `traits/loader.rs`: added `///` on `TemplateLoader` trait (with explicit
  rationale for `#[async_trait]` — `dyn`-compatibility for `DynTemplateLoader`),
  every trait method, `FileSystemLoader` and its public constructors,
  `EmbeddedLoader`. Trait-impl method bodies left undocumented (rustdoc
  inherits the trait-level docs).
- `Cargo.toml`: added `[package.metadata.docs.rs] all-features = true` so the
  `tokio`-gated `FileSystemLoader` is rendered on docs.rs.

No behaviour changes. No `anyhow` was present to remove. No public signatures
changed.

---

## Architectural Compliance

Layer: `shared`. Per `instructions/information/boundaries.md` dependencies must
flow downward only. Crate has a single downward dep on
`systemprompt-provider-contracts` for cross-crate provider traits, which is
sanctioned.

---

## Passing Checks

| Check | Status |
|-------|--------|
| `cargo fmt -p systemprompt-template-provider --check` | PASS |
| `cargo build -p systemprompt-template-provider --all-features` | PASS |
| `cargo clippy -p systemprompt-template-provider --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-template-provider --no-deps --all-features` | PASS |
| `just check-bans-crate systemprompt-template-provider` | PASS |
| Public-API §3a Hygiene: rustdoc on every `pub` item | PASS |
| Public-API §3a Hygiene: `//!` on every `pub mod` | PASS |
| Public-API §3a Hygiene: `thiserror` errors in public signatures | PASS |
| Public-API §3a Hygiene: `[package.metadata.docs.rs] all-features` | PASS |
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 4 |
| Files over 300 lines | 0 |
| Largest file | `crates/shared/template-provider/src/traits/loader.rs` (261 lines) |

---

## Verdict

**CLEAN**

Crate meets the §3a Public-API Hygiene bar for published library code. All
public items carry rustdoc, the error type is `thiserror`-derived, the feature
flag is documented, and the `tokio`-gated surface renders on docs.rs.
