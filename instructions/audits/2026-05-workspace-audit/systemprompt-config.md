# Audit — systemprompt-config

Crate: `crates/infra/config/` — profile-based configuration bootstrap layer.
Date: 2026-05-15.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only on `systemprompt-models`, `systemprompt-traits`, `systemprompt-logging` — all infra/shared layer; no upward deps. |
| 2 | Error model | clean | `thiserror` enums (`ConfigError`, `ProfileBootstrapError`, `SecretsBootstrapError`, `ConfigValidationError`); no `anyhow` in any public signature. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`; `unwrap_or_else` fallbacks only. CLI output goes through `systemprompt_logging::CliService`. |
| 4 | Raw SQL | clean | Crate has no SQL — config-only. |
| 5 | File size | clean | Largest file `manager.rs` 269 lines; all under the 300-line limit. |
| 6 | Function size | clean | All functions under the 75-line guidance; `build_config` (~40) and `generate_config` (~50) are the largest, both cohesive. |
| 7 | Async traits | clean | No async; `DomainConfig` impl is synchronous. |
| 8 | Typed identifiers | clean | No entity IDs in this crate; string fields are file paths and config keys, not identifiers. |
| 9 | Comment standard | remediated | `loader.rs` module head narrated a past refactor ("Split out of `mod.rs` ... file-size budget under 300 lines"); rewritten to describe current behaviour. No paraphrase `///` walls; `//!` heads substantive. |
| 10 | No legacy | clean | No backwards-compat shims or `Option<T>` migration stubs; env-fallback paths are intentional runtime modes (subprocess/Fly.io), not legacy. |
| 11 | Naming | clean | `ConfigManager` is a borderline `*Manager` name but is the established public API surface for the `systemprompt cloud config` pipeline; renaming would be a breaking API change out of scope for a standards refactor. `ConfigValidator`/`ConfigWriter`/`SkillConfigValidator` follow `*Service`/role conventions. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | `read_env_optional` appears in both `loader.rs` and `manager.rs` but in separate module trees (bootstrap vs services) with no shared parent; a 6-line trivial helper — extraction would add cross-module coupling for no real gain. |
| 14 | CHANGELOG accuracy | clean | Existing `[0.9.2]` entry accurately reflects the code (BootstrapSequence, SkillConfigValidator, profile_gateway, ConfigError unification, tightened visibility). The crate is now at workspace version 0.10.x with no new code changes since 0.9.2; no fabricated release notes added. |

## Summary

1 item remediated (comment standard), 13 clean. Clippy and `cargo doc` pass clean with `-D warnings`.
