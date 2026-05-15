# Audit — systemprompt-template-provider

Crate: `crates/shared/template-provider/` — shared layer. Audited 2026-05-15.

1. Layering — clean: depends only on `systemprompt-provider-contracts` (same shared layer) plus `async-trait`, `thiserror`, optional `tokio`. No upward deps.
2. Error model — clean: `thiserror`-derived `TemplateLoaderError` enum in `traits/error.rs`; no `anyhow` in public signatures.
3. No panics — clean: no `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`; all fallible paths return `Result`.
4. Raw SQL — clean: crate contains no SQL.
5. File size — clean: largest file `traits/loader.rs` is 237 lines, under the 300-line limit.
6. Function size — clean: largest fn `FileSystemLoader::load_directory` ~67 lines; sub-steps already extracted into private helpers.
7. Async traits — remediated: `TemplateLoader` uses `#[async_trait]` (correct, consumed as `Arc<dyn TemplateLoader>`); the dyn-compat reason was undocumented and is now stated on the trait.
8. Typed identifiers — clean: no entity IDs; only `PathBuf`/`String` for filesystem paths and template content.
9. Comment standard — remediated: substantive `//!` heads, no `///` paraphrase; fixed three broken intra-doc links to feature-gated `FileSystemLoader` (unresolved when docs built without the `tokio` feature) by demoting them to plain backtick text.
10. No legacy — clean: no compat shims, dual paths, or `Option<T>` stubs.
11. Naming — clean: trait/loader names (`TemplateLoader`, `EmbeddedLoader`, `FileSystemLoader`); no `*Manager`.
12. Tests location — clean: no inline `#[cfg(test)] mod tests`.
13. Local duplication — clean: base-path canonicalisation/validation already factored into `is_within_base_paths` / `canonicalize_and_validate` / `try_read_from_base`.
14. CHANGELOG accuracy — clean: entries reflect real history; 0.0.2/0.0.3 schema-migration notes are workspace-wide release boilerplate, not specific code claims about this crate.
