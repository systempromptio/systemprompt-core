# systemprompt-templates Tech Debt Audit

**Layer:** domain
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave C1)
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
| Doc `///` comments | added on every `pub` item touched |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in PUBLIC signatures | 0 (was 7) |
| `async_trait` references | 0 |

**Total scored violations:** 0

---

## Wave C1 Fixes Applied

- `error.rs`: replaced `anyhow::Error` source fields on `LoadError`/`CompileError`/`RenderError` with plain `String` `message` fields; added `Io(#[from] std::io::Error)` and `Yaml(#[from] serde_yaml::Error)`; introduced `TemplateResult` alias.
- `core_provider.rs`: 4 public `anyhow::Result` signatures (`discover`, `discover_from`, `discover_with_priority`, internal `discover_templates`) converted to `TemplateResult`.
- `registry/lifecycle.rs` and `registry/queries.rs`: error construction updated to use the new `message: e.to_string()` field.
- `lib.rs`: added `//!` crate-level docs with feature-flag matrix and layering notes; `error` module promoted to `pub mod`; `TemplateResult` re-exported.
- `Cargo.toml`: added `[package.metadata.docs.rs] all-features = true`; dropped `anyhow` direct dependency (no longer used).

## sqlx Verification

`grep -E 'sqlx::query[^_!a-zA-Z]' crates/domain/templates/src` → no matches. The crate has no SQL surface.

---

## File Splits

The single file >300 lines flagged in the baseline was already split before Wave C1 began (no >300 line files remain).

---

## Passing Checks

| Check | Status |
|-------|--------|
| `cargo build -p systemprompt-templates --all-features` | PASS |
| `cargo clippy -p systemprompt-templates --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-templates --no-deps --all-features` | PASS |
| `just check-bans-crate systemprompt-templates` | PASS |
| `cargo build --workspace --all-features` | PASS |

---

## Verdict

**CLEAN**
