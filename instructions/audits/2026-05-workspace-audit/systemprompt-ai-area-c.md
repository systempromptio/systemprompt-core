# Audit — systemprompt-ai (Area C: models, repository, jobs, root files)

Scope: `crates/domain/ai/src/{models,repository,jobs}/`, root `src/{lib.rs,error.rs,extension.rs}`,
and `CHANGELOG.md`. Excludes `src/services/` (audited concurrently by other agents).

1. **Layering** — clean. Models/repository depend only downward (`systemprompt-database`,
   `systemprompt-models`, `systemprompt-identifiers`, `systemprompt-provider-contracts`).
2. **Error model** — clean. `error.rs` is `thiserror`-derived (`AiError`, `RepositoryError`);
   no `anyhow` in any public signature.
3. **No panics** — clean. No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in scope.
4. **Raw SQL** — clean. Repositories use only `sqlx::query!`/`query_scalar!` compile-time macros.
5. **File size** — clean. Largest in-scope file is `ai_request_record.rs` at 282 lines (<300).
6. **Function size** — clean. No in-scope function exceeds the ~75-line guidance.
7. **Async traits** — clean. No `#[async_trait]`; no trait definitions in scope.
8. **Typed identifiers** — remediated (partial). Repository/record types use typed IDs
   correctly. Deviation noted: `models/image_generation.rs` (`GeneratedImageRecord.uuid`,
   `request_id`, `trace_id`; `ImageGenerationRequest.trace_id`, `mcp_execution_id`) and
   `models/mod.rs` (`AiRequestMessage.id`, `AiRequestToolCall.id`) retain raw `String`.
   Converting these crosses into `src/services/` (owned by other agents) and would be a
   non-safe behavioural/signature change — left untouched per scope rules; flagged for a
   follow-up coordinated change.
9. **Comment standard** — clean. `lib.rs`/`error.rs`/`models/mod.rs`/`repository/mod.rs`
   carry substantive `//!` heads (purpose, surface, error model). No `///` paraphrase smells.
10. **No legacy** — remediated. Removed dead empty module `src/jobs/mod.rs` (a leftover
    stub from the removed `evaluations` feature; `pub(crate)` and referenced nowhere) and
    its `mod jobs;` declaration in `lib.rs`.
11. **Naming** — clean. No `*Manager` types; repositories named `*Repository`.
12. **Tests location** — clean. No inline `#[cfg(test)] mod tests` in scope.
13. **Local duplication** — clean. No significant in-scope duplication.
14. **CHANGELOG accuracy** — clean. `0.10.2` entry (resilience layer, `AiError::HttpStatus`/
    `Timeout`/`CircuitOpen`/`DependencyUnavailable`, `classify`) matches `error.rs`.
