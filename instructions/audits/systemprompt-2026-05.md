# systemprompt Tech Debt Audit

**Layer:** facade
**Audited:** 2026-05-04
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
| Doc `///` comments on `pub` items | required (present everywhere) |
| Doc `//!` module / crate header | required (present) |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 1 (`clippy::doc_markdown` carried over from baseline; required because rustdoc tables and bullet lists trip the check) |
| `anyhow::` references | 0 |
| `async_trait` references | 0 |

**Total scored violations:** 0

---

## §3a Public-API Hygiene Compliance

| Rule | Status |
|------|--------|
| Rustdoc on every `pub` item (`cargo doc -D warnings`) | PASS |
| Module-level `//!` docs | PASS — `lib.rs`, `runtime.rs`, `prelude.rs` |
| Public errors typed (no `anyhow::Error` in signatures) | PASS — `RuntimeError` (`thiserror`) replaces previous `anyhow::Result` |
| Feature-flag matrix in `lib.rs` | PASS — `//!` markdown table mapping feature → crates → use case |
| `[package.metadata.docs.rs]` block | PASS — `all-features = true`, `rustdoc-args = ["--cfg", "docsrs"]` |
| Examples per major feature | PASS — `examples/extension.rs`, `examples/database.rs`, `examples/api.rs`, `examples/cli.rs`, gated via `[[example]] required-features` |
| README inclusion via `#![doc = include_str!("../README.md")]` | PASS — `lib.rs` line 3 |
| Re-export rustdoc | PASS — every `pub use` and every `pub mod` carries a `///` line |

---

## Architectural Compliance

Layer: `facade`. Per `instructions/information/boundaries.md` dependencies must
flow downward only — the facade legitimately depends on every published crate
in the workspace.

---

## Verification Gates

| Command | Status |
|---------|--------|
| `cargo fmt -p systemprompt` | PASS |
| `cargo build -p systemprompt --all-features` | PASS |
| `cargo clippy -p systemprompt --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt --no-deps --all-features` | PASS |
| `cargo check -p systemprompt --example extension --features core` | PASS |
| `cargo check -p systemprompt --example database --features database` | PASS |
| `cargo check -p systemprompt --example api --features api` | PASS |
| `cargo check -p systemprompt --example cli --features cli` | PASS |
| `just check-bans-crate systemprompt` | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 7 (3 src + 4 examples) |
| Files over 300 lines | 0 |
| Largest file | 295 lines (`systemprompt/src/lib.rs`) |
| Examples added | 4 |

---

## Wave E3 Changes

1. Removed all `anyhow` usage from the facade (`Cargo.toml`, `runtime.rs`).
   Introduced `RuntimeError` (`thiserror`) with `ExtensionsAlreadyInjected`
   and `Cli(Box<dyn Error + Send + Sync>)` variants.
2. Added rustdoc `///` to every `pub use` and `pub mod` in `lib.rs`,
   `prelude.rs`, and `runtime.rs`. Each line states what the re-export
   provides.
3. Added the `//!` feature-flag matrix at the top of `lib.rs` (markdown
   table, 16 rows covering every flag).
4. Added `[package.metadata.docs.rs]` block to `Cargo.toml` with
   `all-features = true` and `rustdoc-args = ["--cfg", "docsrs"]`.
5. Added `examples/extension.rs`, `examples/database.rs`,
   `examples/api.rs`, `examples/cli.rs`, each gated via
   `[[example]] required-features` so they only compile under the right flag.
6. Added `#![doc = include_str!("../../README.md")]` so docs.rs renders the
   README on the crate landing page. Patched README intra-doc links
   (`LICENSE`, `SECURITY.md`) to absolute GitHub URLs and tagged the ASCII
   architecture block as `text` so rustdoc accepts it.
7. Promoted `prelude` from a private module to `pub mod` so examples and
   downstream consumers can reach it via `systemprompt::prelude::*`.
8. Trimmed module-level rustdoc to keep `lib.rs` under the 300-line bar
   (final 295 lines).

---

## Verdict

**CLEAN**
