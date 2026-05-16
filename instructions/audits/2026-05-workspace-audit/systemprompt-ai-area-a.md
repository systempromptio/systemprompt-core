# systemprompt-ai audit — Area A: `src/services/providers/`

Scope: `crates/domain/ai/src/services/providers/` only. 41 source files audited.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | No upward/cross-layer deps; only `crate::*`, `systemprompt_models`, `systemprompt_identifiers`. |
| 2 | Error model | clean | All public fns return `crate::error::Result` (`thiserror` `AiError`); no `anyhow`. |
| 3 | No panics | remediated | Replaced `unreachable!()` in `anthropic/converters.rs` with explicit per-role match arms. No `unwrap`/`expect`/`println!` found. |
| 4 | Raw SQL | clean | No SQL anywhere in this provider tree. |
| 5 | File size | remediated | `anthropic/generation.rs` (293 lines, heavy 3x duplication) split into `request.rs` + `response.rs`; now 196 lines. |
| 6 | Function size | remediated | Per-function request/response duplication extracted; `*_helpers.rs` padding file `gemini_images_helpers.rs` renamed to cohesive `gemini_image_mapping.rs`. |
| 7 | Async traits | remediated | `#[async_trait]` on `AiProvider` and `ImageProvider` is required (boxed `dyn` dispatch); added `// Why:` reason comments on both trait defs. |
| 8 | Typed identifiers | clean | `AiToolCallId::new(...)` used; no raw `String` IDs or `.into()`/`::from()` at call sites. |
| 9 | Comment standard | clean | `//!` heads substantive; no `///` paraphrase walls; no narration comments. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | No `*Manager`; types are `*Provider`/`*Factory`/`*Service`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | remediated | Anthropic sampling-tuple extraction, HTTP POST+status-check, and `AiResponse` construction were repeated across `generation.rs` (3x) and `streaming.rs`; extracted into shared `request.rs`/`response.rs` and reused. |
| 14 | CHANGELOG | clean | Not edited (observations only). |

## Notes

- `anthropic/search.rs` deliberately not folded into shared `post_messages`: it
  uses a different error path (`AiError::Internal` with response body text)
  rather than `from_error_response`. Behaviour preserved by leaving it alone.
- All refactors are behaviour-preserving: provider request/response semantics
  for Anthropic generation, tool, schema, and streaming calls are byte-identical.
- Verified: `cargo clippy -p systemprompt-ai --all-targets --all-features -D warnings`
  and `cargo doc -p systemprompt-ai --no-deps` both clean (`SQLX_OFFLINE=true`).
