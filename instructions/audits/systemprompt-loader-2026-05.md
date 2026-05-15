# systemprompt-loader Tech Debt Audit

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
| `let _ =` discards | 1 | 0 |
| `.ok()` discards | 2 | 0 (replaced with logged `mtime_of` helper) |
| Inline `//` comments | 0 | 0 |
| Doc `///` coverage on pub items | 0 / 53 | 53 / 53 |
| Files >300 lines | 1 (config_loader.rs at 426) | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |
| `anyhow::` references in public API | 17 | 0 |
| `async_trait` references | 0 | 0 |

**Total scored violations:** 0

---

## Wave B1 Fixes Applied

### Typed errors

- Introduced `error.rs` with thiserror-derived enums:
  - `ConfigLoadError` — services-config I/O, YAML parsing, include
    cycle / not-found, duplicate definitions, validation failures.
  - `ConfigWriteError` — agent file CRUD failures, with `#[from]`
    `serde_yaml::Error`.
  - `ExtensionLoadError` — registry binary / manifest lookup failures.
  - `ProfileLoadError` — composes `systemprompt_models::profile::ProfileError`
    via `#[from]` plus crate-local `Io { path, source }`.
- Replaced every `anyhow::Result` in public signatures across
  `ConfigLoader`, `ConfigWriter`, `ExtensionRegistry`, and
  `ProfileLoader` with the matching typed `*Result` aliases.
- Dropped `anyhow` from `Cargo.toml`.

### File-size split (config_loader.rs: 426 → 213)

`config_loader.rs` was split into a `config_loader/` module:

- `config_loader/mod.rs` — public `ConfigLoader` API (213 lines).
- `config_loader/types.rs` — `RootConfig`, `PartialServicesFile`,
  `IncludeResolveCtx` (77 lines).
- `config_loader/includes.rs` — recursive include resolver (62 lines).
- `config_loader/merge.rs` — `merge_partial`, `merge_skills`,
  `merge_content`, `resolve_partial_includes`,
  `resolve_system_prompt_includes`, `resolve_skill_instruction_includes`
  (197 lines).

`extension_loader.rs` exceeded 300 lines after rustdoc was added, so it
was likewise split:

- `extension_loader/mod.rs` — public `ExtensionLoader` API (256 lines).
- `extension_loader/manifest.rs` — `ManifestLoadError` (private) and
  `load_manifest` / `mtime_of` helpers (45 lines).
- `extension_loader/result.rs` — `ExtensionValidationResult` (33 lines).

### `let _ =` and `.ok()` cleanup

- `config_loader.rs:119` `let _ = Self::load_from_path(path)?;` removed;
  `validate_file` now reads `Self::load_from_path(path).map(|_| ())`.
- `extension_loader.rs:138-139` two `.ok()` discards on `fs::metadata(...)`
  replaced with a single `mtime_of` helper that logs at `debug` level
  via `tracing::debug!` before returning `None`.

### Documentation

- Added crate-level `//!` with module map and feature-flag matrix.
- Added `//!` module docs to every `pub mod`.
- Added `///` doc comments to every `pub` item (functions, structs,
  enums, fields, type aliases, constants, error variants).
- Added `[package.metadata.docs.rs] all-features = true` to
  `Cargo.toml`.

### Downstream caller fix-ups

To absorb the new typed errors via `?`:

- `crates/domain/agent/src/error.rs` — added
  `ServicesConfig(#[from] systemprompt_loader::ConfigLoadError)` variant.
- `crates/domain/mcp/src/error.rs` — added `ServicesConfig(#[from]
  ConfigLoadError)` and `ExtensionLoad(#[from] ExtensionLoadError)`.
- `crates/domain/mcp/src/services/deployment/mod.rs::load_config` — explicit
  `.map_err(anyhow::Error::from)` (the surrounding caller still uses
  `anyhow::Result`).
- `crates/entry/cli/src/shared/profile.rs` — wraps `ProfileLoadError` in
  `anyhow::Error::from` where the caller's variant carries
  `anyhow::Error`.

These are the minimum changes required to keep the workspace compiling;
they do not introduce any new tech debt.

---

## Architectural Compliance

Layer: `infra`. Per `instructions/information/boundaries.md` dependencies
flow downward only; this crate imports from `shared/*` (`models`,
`extension`) and `infra/config` only.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | PASS |
| No `.ok()` silent discards | PASS |
| All pub items carry `///` rustdoc | PASS |
| All `pub mod` carry `//!` rustdoc | PASS |
| All files <=300 lines | PASS |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | PASS |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | PASS |
| No `anyhow::` in public signatures | PASS |
| `cargo fmt -p systemprompt-loader --check` | PASS |
| `cargo build -p systemprompt-loader --all-features` | PASS |
| `cargo clippy -p systemprompt-loader --lib --all-features --no-deps -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-loader --no-deps --all-features` | PASS |
| `just check-bans-crate systemprompt-loader` | PASS |

`--all-targets` clippy is currently unable to run cleanly because the
unrelated `systemprompt-logging::services::cli` crate violates the new
`let_underscore_must_use` workspace lint introduced in commit
`87d901a0`; that crate is owned by Wave B's logging slice, not this
slice.

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 14 (was 8 — added `error.rs` plus the two split modules) |
| Files over 300 lines | 0 |
| Largest file | `extension_loader/mod.rs` (256 lines) |

---

## Verdict

**CLEAN**
