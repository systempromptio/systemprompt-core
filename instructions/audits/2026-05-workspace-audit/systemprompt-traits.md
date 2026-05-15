# Audit — systemprompt-traits (`crates/shared/traits/`)

Date: 2026-05-15. Scope: standards audit + safe single-crate remediation.

1. **Layering** — `clean`. Depends only on `systemprompt-identifiers` and `systemprompt-provider-contracts` (both `shared` layer). No upward/cross-layer deps.
2. **Error model** — `clean`. Every provider trait pairs with a `thiserror`-derived enum; no `anyhow` anywhere. `pub type Result` uses `Box<dyn Error + Send + Sync>`, not `anyhow::Error`.
3. **No panics** — `clean`. No `unwrap`/`expect`/`panic!`/`todo!`/`dbg!`/`println!`/`eprintln!` in `src/`.
4. **Raw SQL** — `clean`. No `sqlx::query` usage; this is an interface-only crate.
5. **File size** — `clean`. Largest file is `startup_events/ext.rs` at 212 lines; all under the 300-line limit.
6. **Function size** — `clean`. Trait method bodies are default/empty or trivial; no function exceeds the 75-line guidance.
7. **Async traits** — `clean`. All `#[async_trait]` traits are consumed as `Arc<dyn …>` (each has a `Dyn*` alias); the `lib.rs` `//!` "Async traits" section documents the `dyn`-compatibility rationale for the whole surface.
8. **Typed identifiers** — `clean (noted)`. Most traits use typed IDs (`&UserId`, `&SessionId`, `ContentId`, `SourceId`, `FileId`). Three residual raw-`str`/`String` IDs remain: `ContentProvider::get_content(id: &str)` and `ContentItem/ContentFilter.category_id: Option<String>` (`content.rs`), and `LogService::get_by_id`/`delete(id: &str)` (`log_service.rs`). Not remediated here: both traits are implemented in `domain/content` and `infra/logging`; migrating their signatures requires a coordinated cross-crate change that cannot be verified within this isolated single-crate audit. `ImageGenerationInfo.request_id: Option<String>` is an external provider's opaque response field — `String` is correct.
9. **Comment standard** — `remediated`. No `///` paraphrase comments and no inline `//` comments anywhere; every module file carries a substantive `//!` head. Fixed a broken intra-doc link in the `lib.rs` feature matrix (`[`ApiModule`]` referenced a `web`-feature-gated item, breaking `cargo doc` without `--all-features`); demoted to plain code text.
10. **No legacy** — `clean`. No backwards-compat shims, dual code paths, deprecation stubs, or `Option<T>` migration stubs.
11. **Naming** — `clean`. `*Provider`/`*Service`/`*Module` naming throughout; no `*Manager`.
12. **Tests location** — `clean`. No inline `#[cfg(test)] mod tests`.
13. **Local duplication** — `clean`. No repeated logic warranting extraction.
14. **CHANGELOG accuracy** — `clean`. The 0.2.0 entry (typed-ID migration of `ContextProvider`/`UserProvider`/`RoleProvider`, removal of `AuthProvider`/`AuthorizationProvider` and associated types) matches the current code.

## Outcome

One doc-link fix applied (item 9). Item 8's residual cross-crate typed-ID migration is
documented for a future coordinated change. All other items clean.
