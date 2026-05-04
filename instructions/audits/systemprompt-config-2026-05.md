# systemprompt-config Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave B2)
**Verdict:** CLEAN

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 2 (build.rs helpers, scoped via `#[expect]`) |
| `let _ =` discards | 0 |
| `.ok()` discards | 0 (file-source `std::env::var(...).ok()` paths replaced with explicit `read_env_optional` helper) |
| Inline `//` comments | 0 |
| Doc `///` comments | covered for every `pub` item |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 1 (`ConfigManager`) — kept; not domain orchestration, deployment-config builder for the `systemprompt cloud config` flow |
| `#[allow(...)]` | 0 (replaced with two scoped `#[expect(...)]` on `build_validate_configs` build-script helper) |
| `anyhow::` references | 0 |
| `async_trait` references | 0 |

**Total scored violations:** 0

---

## Wave B2 Compliance Sweep

### Public-API hygiene

- All `anyhow::Result` / `anyhow::Error` removed from public signatures.
- New crate-level `ConfigError` enum at `src/error.rs` composes `ProfileBootstrapError`, `SecretsBootstrapError`, `ProfileError`, `GatewayProfileError`, `ConfigValidationError`, `SecretsError`, plus `std::io`, `serde_json`, `serde_yaml`, and `regex` via `#[from]`.
- Public alias `ConfigResult<T> = Result<T, ConfigError>` re-exported from `lib.rs`.
- `[package.metadata.docs.rs]` block added with `all-features = true` and `--cfg docsrs`.
- `//!` crate-level rustdoc with public-surface map and feature-flag note.
- `///` rustdoc on every `pub` item; `# Errors` sections on every fallible API.

### File splits

`bootstrap/secrets.rs` (403 lines) split into a module directory:
- `bootstrap/secrets/mod.rs` — singleton + `SecretsBootstrap` state machine.
- `bootstrap/secrets/loader.rs` — file/env loaders.
- `bootstrap/secrets/logging.rs` — diagnostics + `build_loaded_secrets_message`.
- `bootstrap/secrets/io.rs` — `load_secrets_from_path` + `handle_load_error`.

`services/validator.rs` (313 lines after rewrite) trimmed to 248 lines by extracting `ValidationReport` to `services/report.rs`.

### Other fixes

- 18 `.ok()` calls in `bootstrap/secrets/loader.rs` and `services/manager.rs` replaced with `read_env_optional`/`read_env_required` helpers that distinguish "missing" from "empty".
- `#[allow(clippy::print_stderr, clippy::exit, clippy::print_stdout, clippy::type_complexity)]` on `build_validate_configs` removed; replaced with scoped `#[expect]` on the two private helpers `emit_rerun` / `emit_failure`, plus a public `ConfigValidatorFn` type alias to satisfy `clippy::type_complexity`.

---

## Self-verification gate

- `cargo fmt -p systemprompt-config` — clean.
- `cargo build -p systemprompt-config` — green.
- `cargo clippy -p systemprompt-config --no-deps --all-targets -- -D warnings` — green.
- `cargo doc -p systemprompt-config --no-deps` with `RUSTDOCFLAGS="-D warnings"` — green.
- Workspace `cargo build --workspace` — green (consumers transparently pick up `From<ConfigError>` via `?`).

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 19 |
| Files over 300 lines | 0 |
| Largest file | `services/manager.rs` (289) |

---

## Verdict

**CLEAN** — ready for crates.io publication after the wave merges.
