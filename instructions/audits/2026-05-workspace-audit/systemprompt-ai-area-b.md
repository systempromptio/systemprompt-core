# Audit — systemprompt-ai, Area B

Scope: `crates/domain/ai/src/services/{core,schema,tooled,tools,structured_output,storage,config}/` plus `services/mod.rs`.

1. Layering — clean. Only depends on crate-internal modules and `systemprompt_{models,traits,identifiers}`.
2. Error model — clean. `crate::error::AiError` (thiserror) used; no `anyhow` in any signature.
3. No panics — clean. No `unwrap()`/`expect()`/`panic!`/`dbg!`/`println!`/`eprintln!`; only `unwrap_or`/`unwrap_or_else` fallbacks.
4. Raw SQL — clean. All persistence routed through `AiRequestRepository`; no `sqlx::query` in scope.
5. File size — clean. Largest file 244 lines, under the 300-line limit.
6. Function size — remediated. `ConfigValidator::validate_providers` was 78 lines; extracted the no-providers help-message branch into private `no_providers_message`.
7. Async traits — clean. `#[async_trait]` only on impls of external `dyn`-compatible traits (`AiProvider`, `ToolProvider`); trait definitions out of scope.
8. Typed identifiers — clean. Typed IDs constructed via `Id::new`. Raw `String` fields (`ToolCallData.ai_tool_call_id`, external trait struct fields) are local DTO / external-trait surfaces, not service args.
9. Comment standard — clean. Substantive `//!` heads on all module files; no `///` paraphrase and no WHAT `//` comments.
10. No legacy — clean. No shims, dual paths, or `Option<T>` migration stubs.
11. Naming — clean. `*Service`/`*Validator`/`*Storage`/`*Formatter`; no `*Manager`.
12. Tests location — clean. No inline `#[cfg(test)] mod tests`.
13. Local duplication — clean. No notable repeated blocks.
14. CHANGELOG — observations only; not edited.
