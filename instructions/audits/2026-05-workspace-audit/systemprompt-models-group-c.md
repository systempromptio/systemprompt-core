# Audit — systemprompt-models (Group C)

Scope: `crates/shared/models/` excluding `artifacts/`, `services/`, `api/`,
`profile/`, `ai/`, `a2a/`, `execution/`. Covers `lib.rs`, root `.rs` files,
and `admin/ agui/ auth/ bridge/ config/ content/ errors/ events/ extension/
mcp/ modules/ oauth/ paths/ repository/ routing/ users/ validators/`.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Foundation `shared/*` crate; only depends on `shared/*` siblings. |
| 2 | Error model | clean | `thiserror` enums in `errors/`; no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in scope. |
| 4 | Raw SQL | clean | No `sqlx::query` calls; this crate holds models only. |
| 5 | File size | clean | Largest in-scope file 277 lines (`bridge/ids.rs`); all under 300. |
| 6 | Function size | clean | No in-scope function exceeds the ~75-line guidance. |
| 7 | Async traits | remediated | Documented the `dyn`-compat reason for `#[async_trait]` on `mcp/registry_trait.rs` and `repository/service.rs`. |
| 8 | Typed identifiers | clean | Struct ID fields use `systemprompt_identifiers` types; JWT claim `String`s are external-protocol fields (spec carve-out). |
| 9 | Comment standard | remediated | Replaced 18 placeholder `//! ... see crate-level docs` heads with substantive module docs (purpose/surface); `lib.rs` head already complete. |
| 10 | No legacy | clean | No backwards-compat shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | No `*Manager`; `RouteClassifier`/`*Config`/`*Service` consistent. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No material repeated logic warranting extraction. |
| 14 | CHANGELOG accuracy | clean | Observation only; `CHANGELOG.md` not edited (owned elsewhere). |
