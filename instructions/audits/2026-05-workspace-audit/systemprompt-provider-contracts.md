# Audit — systemprompt-provider-contracts

Crate: `crates/shared/provider-contracts/` — shared-layer trait contract crate.

1. Layering — **clean**: only in-workspace dep is `systemprompt-identifiers` (shared layer); all others are external crates.
2. Error model — **clean**: four `thiserror` enums (`ProviderError`, `LlmProviderError`, `ToolProviderError`, `WebConfigError`); no `anyhow` anywhere.
3. No panics — **clean**: no `unwrap`/`expect`/`panic!`/`todo!`/`dbg!`/`println!`/`eprintln!`.
4. Raw SQL — **clean**: crate is contract-only; no SQL at all.
5. File size — **clean**: largest source file is 177 lines (`job.rs`), well under the 300-line limit.
6. Function size — **clean**: no function exceeds the ~75-line guidance.
7. Async traits — **remediated**: 12 `#[async_trait]` traits were undocumented; added a `// Why:` line on each explaining the trait is consumed as a trait object (`dyn`/`Arc<dyn>`/`inventory`-collected), making `#[async_trait]` mandatory.
8. Typed identifiers — **clean**: ID fields use `SourceId`/`SessionId`/`TraceId`/`AiToolCallId`/`LocaleCode`; `.into()` call sites convert only `String`/`PathBuf` in builders, not typed IDs.
9. Comment standard — **clean**: substantive `//!` heads on `lib.rs` and every module; no `///` paraphrase noise; no narrative inline comments.
10. No legacy — **clean**: no compat shims, dual paths, or `Option<T>` migration stubs.
11. Naming — **clean**: types are `*Provider`/`*Processor`/`*Renderer`/`*Context`/`*Service`-free of `*Manager`.
12. Tests location — **clean**: no inline `#[cfg(test)] mod tests`.
13. Local duplication — **clean**: builder/`Debug` impls are per-type and not extractable without coupling.
14. CHANGELOG accuracy — **clean**: recent entries (`0.9.2` format normalization) match the code state.

Remediated: 1 of 14 (item 7). Clean: 13 of 14.
