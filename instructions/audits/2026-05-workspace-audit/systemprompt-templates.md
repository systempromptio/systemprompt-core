# Audit: systemprompt-templates

Crate: `crates/domain/templates/` — version 0.10.2. Domain layer.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only systemprompt dep is `systemprompt-template-provider` (shared); flows downward. |
| 2 | Error model | clean | `TemplateError` is a `thiserror` enum in `error.rs`; no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in `src/`. |
| 4 | Raw SQL | clean | No database access; crate is a Handlebars engine. |
| 5 | File size | clean | Largest file `core_provider.rs` at 169 lines; all under 300. |
| 6 | Function size | clean | All functions well under the 75-line guidance. |
| 7 | Async traits | clean | `TemplateProvider` impls use plain `fn`; no `#[async_trait]`. Unused `async-trait` dep removed (item 10). |
| 8 | Typed identifiers | clean | No entity IDs; template names are inherently free-form strings, not typed IDs. |
| 9 | Comment standard | clean | Substantive `//!` heads on `lib.rs` and `core_provider.rs`; no `///` paraphrase, no WHAT/narration comments. |
| 10 | No legacy | remediated | Removed unused `async-trait` and `indexmap` regular deps, and the dead `[dev-dependencies]` block (tests live in the external `crates/tests/` workspace). |
| 11 | Naming | clean | `TemplateRegistry`, `CoreTemplateProvider`, `TemplateRegistryBuilder`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)]`; unit tests in `crates/tests/unit/domain/templates/`. |
| 13 | Local duplication | clean | `extenders_for`/`components_for`/`page_providers_for` share an `applies_to` filter shape but operate on distinct trait objects with no extractable common type; left as-is. |
| 14 | CHANGELOG accuracy | clean | 0.9.2 entries (`EmbeddedDefaultsProvider`, `json` helper, `RegistryStats`, registry split) match the code. |

## Remediation summary

Single change: pruned unused dependencies from `Cargo.toml` (`async-trait`, `indexmap`, and the unused `[dev-dependencies]` `tokio`/`tempfile`). No source or behavioural changes. Clippy and doc verified clean.
