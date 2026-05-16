# Audit: `systemprompt` facade crate

Crate at repo-root `systemprompt/`. Public facade — re-exports workspace crates behind feature flags (`core`, `database`, `api`, `cli`, `full`, plus granular infra/domain features). Files: `lib.rs`, `prelude.rs`, `runtime.rs`.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Pure re-exports plus one embedding helper (`RuntimeBuilder`); no sideways logic. |
| 2 | Error model | clean | Only facade-defined error is `RuntimeError` (`thiserror`); no `anyhow` in any signature. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in `src/`. |
| 4 | Raw SQL | clean | None — facade has no DB access. |
| 5 | File size | clean | Largest source file `lib.rs` at 301 lines, under the 300-line cohesion proxy after edits (was 305). |
| 6 | Function size | clean | All functions in `runtime.rs` well under 75 lines. |
| 7 | Async traits | clean | Facade defines no traits. |
| 8 | Typed identifiers | clean | Facade defines no ID-bearing structs. |
| 9 | Comment standard | clean | `lib.rs` `//!` states purpose + full feature matrix + prelude usage; `///` on `pub mod` blocks add genuine non-obvious value (which upstream crate, what it provides) — permitted by the facade re-export exception. No inline WHAT-comments. |
| 10 | No legacy | remediated | Removed duplicate top-level `pub use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError}` — the `credentials` module already re-exports the same two items (duplicate re-export path). |
| 11 | Naming | clean | Re-exports keep upstream names; `RuntimeBuilder` correctly suffixed. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | remediated | Same fix as #10 — the `credentials` module and the crate-root `pub use` were identical re-export blocks. |
| 14 | CHANGELOG / feature accuracy | remediated (partial) | `Cargo.toml`: removed dead deps `chrono`, `serde`, `serde_json`, `tracing` (facade has no logic; none referenced in `src/`). `lib.rs` matrix: corrected stale install snippet `version = "0.5"` → `"0.10"` and the `test-utils` row that overclaimed "credential fixtures" (the feature only enables `cloud`). Every `Cargo.toml` feature maps to a `#[cfg(feature = ...)]` in `lib.rs`/`prelude.rs`; matrix agrees with `Cargo.toml`. **Open finding (not remediated):** `CHANGELOG.md` is stale — newest entry is `0.1.18`, but the workspace is `0.10.2`; the `0.2.x`–`0.10.x` release history is absent. Backfilling release notes is outside a standards-audit scope and not derivable from the diff; flagged for the release owner.
