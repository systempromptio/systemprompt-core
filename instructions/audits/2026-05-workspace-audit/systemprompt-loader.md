# Audit — systemprompt-loader

Crate: `crates/infra/loader/` — file and module discovery infrastructure (infra layer).
Date: 2026-05-15.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only on `systemprompt-config`, `-extension`, `-identifiers`, `-models` — all shared/infra layers below; no upward deps. |
| 2 | Error model | clean | Four `thiserror` enums in `error.rs`; no `anyhow` anywhere in the crate. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`; fallible paths return `Result`, `.ok()` uses are env-var/`read_dir` with `tracing::warn!`. |
| 4 | Raw SQL | clean | Crate touches no database; no `sqlx` usage at all. |
| 5 | File size | clean | Largest file `extension_loader/mod.rs` at 227 lines; all under the 300-line limit. |
| 6 | Function size | clean | No function exceeds ~75 lines; `run_from_content` and `discover_marketplaces` are the longest and stay well within. |
| 7 | Async traits | clean | No traits and no async code in the crate. |
| 8 | Typed identifiers | clean | No raw `String` entity IDs in fields/args; `name` parameters are config-map keys, marketplace IDs use the typed `id.as_str()`. |
| 9 | Comment standard | remediated | `lib.rs` `//!` falsely claimed "no Cargo features" — corrected to document `expose-internals`. Existing `///` on private helpers encode genuine *why* and were kept. |
| 10 | No legacy | clean | No shims or dual paths; `0.2.0` breaking removals already landed. `#[cfg(feature = "expose-internals")]` is a deliberate, documented gate, not a stub. |
| 11 | Naming | clean | `*Loader`/`*Writer`/`*Registry` types; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | remediated | Removed the trivial `read_include` wrapper (a no-op delegate to `fs::read_to_string`); inlined both call sites. |
| 14 | CHANGELOG accuracy | clean | `0.9.2` entries (the `expose-internals` feature and the `config_loader`/`extension_loader` submodule split) match the current code. |

## Remediation summary

- `lib.rs`: fixed the feature-flag `//!` block, which incorrectly stated the crate had no Cargo features.
- `config_loader/merge.rs`: removed the single-line `read_include` indirection; both callers now use `fs::read_to_string` directly.
- `error.rs`: grouped the `ProfileLoadError` enum with the other enum definitions ahead of the `*Result` type aliases (cosmetic ordering).

Verification: `cargo clippy -p systemprompt-loader --all-targets --all-features -- -D warnings` and
`cargo doc -p systemprompt-loader --no-deps` both clean. (A pre-existing `future_not_send`
clippy error in the transitive `systemprompt-database` dependency is out of scope for this crate.)
