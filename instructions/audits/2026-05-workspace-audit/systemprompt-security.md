# Audit — `systemprompt-security` (`crates/infra/security/`)

Date: 2026-05-15. Standards: `rust-coding-standards` skill + `CLAUDE.md`.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Deps are infra/shared only (`systemprompt-models`, `-config`, `-database`, `-extension`, `-identifiers`); no upward/cross-layer deps. |
| 2 | Error model | clean | `thiserror` enums (`AuthError`, `JwtError`, `ManifestSigningError`, `AuthzError`, `AuthzBootstrapError`); no `anyhow` in any public signature. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`; `OnceLock::set` result is explicitly `drop`-ed. |
| 4 | Raw SQL | clean | All SQL via verified `query!`/`query_as!` macros in `repository.rs`, `audit/repository.rs`, `ingestion.rs`; no runtime `sqlx::query(_)`. |
| 5 | File size | clean | Largest file `authz/repository.rs` at 295 lines, under the 300-line limit. |
| 6 | Function size | clean | All functions within ~75-line guidance; resolver/ingestion already extract cohesive private helpers. |
| 7 | Async traits | remediated | `#[async_trait]` on `AuthzDecisionHook` and `AuthzAuditSink` is correct (both stored as `Arc<dyn ...>`); added the required `dyn`-compatibility rationale on each trait. |
| 8 | Typed identifiers | clean | `UserId`/`SessionId`/`RuleId`/`TraceId` etc. used throughout; all `.into()` calls are on plain `String` (cookie names, column names, reason strings), not typed IDs. |
| 9 | Comment standard | clean | Substantive `//!` heads on all modules; per-item `///` is non-obvious value only; one justified `// JSON:` boundary comment in `types.rs`. |
| 10 | No legacy | clean | No backwards-compat shims, dual code paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `*Service`/`*Generator`/`*Validator`/`*Extractor`/`*Repository`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests` in the crate. |
| 13 | Local duplication | remediated | Cookie-parsing loop was duplicated between `extraction/token.rs` and `extraction/cookie.rs`; `TokenExtractor::extract_from_cookie` now delegates to `CookieExtractor`. |
| 14 | CHANGELOG accuracy | clean | `0.9.2` entry accurately describes the present authz module, audit sinks, hook-token validator, and `JwtAudience::Cowork`; 0.10.x bumps were workspace-wide build-script changes with no security-specific surface change to record. |
