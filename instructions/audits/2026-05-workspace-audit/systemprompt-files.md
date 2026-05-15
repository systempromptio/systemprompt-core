# Audit — systemprompt-files

Crate: `crates/domain/files/` — domain layer. Audited 2026-05-15.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Deps flow downward only (database, cloud, config, models, traits, provider-contracts, extension); no upward/cross-layer deps. |
| 2 | Error model | clean | `thiserror`/`domain_error!` enums (`FilesError`, `FileUploadError`, `FileValidationError`); no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`; `unwrap_or_else` with logged fallback in `file_ingestion.rs` is safe. |
| 4 | Raw SQL | clean | All queries via `query!`/`query_as!`/`query_scalar!`; no runtime `sqlx::query(_)`. |
| 5 | File size | clean | Largest source file is `file_ingestion.rs` at 254 lines; all under the 300-line limit. |
| 6 | Function size | clean | `upload_file` (~90 lines) is the longest; cohesive, no padding helpers. Within tolerance. |
| 7 | Async traits | remediated | `#[async_trait]` on `Job`, `AiFilePersistenceProvider`, `FileUploadProvider` impls — all `dyn`-dispatched external traits. Added a WHY note on the local `FileIngestionJob` impl; the trait definitions live in other crates. |
| 8 | Typed identifiers | clean | Service args use typed IDs. `File.id`/`ContentFile.id`/`file_id` remain raw `uuid::Uuid`/`i32`: these are `FromRow` columns and public fields consumed by other crates (`AiGeneratedFile` mirrors them); changing them is a cross-crate public-API break — reported, not changed. `File::id()` accessor returns the typed `FileId`. |
| 9 | Comment standard | remediated | Added `//!` module head to `extension.rs`. No `///` paraphrase noise; no "what we changed" narration found. |
| 10 | No legacy | clean | No backwards-compat shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `*Service`/`*Repository`/`*Validator`/`*Provider`/`*Job`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | `query_as!` column lists repeat across repository methods but are query-literal text, not extractable logic. UUID-parse guard repeated but each carries a distinct error message. |
| 14 | CHANGELOG accuracy | clean | Entries match code. CHANGELOG top is `0.9.2` while workspace pins `0.10.1` — a workspace-wide version-sync artifact, not a per-crate content error. |

## Cross-crate items reported (not changed)

- `File.id`, `ContentFile.id`, `ContentFile.file_id`, `ContentFile.role` use raw
  `uuid::Uuid` / `i32` / `String`. These are `sqlx::FromRow` targets and public
  struct fields; `AiGeneratedFile` in `systemprompt-traits` mirrors the same
  shape. Migrating to typed IDs requires a coordinated change across
  `systemprompt-traits` and consumers — out of scope for a single-crate
  standards pass.
