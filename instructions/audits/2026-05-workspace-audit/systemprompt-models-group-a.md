# systemprompt-models — Group A audit (`artifacts/`, `services/`, `api/`)

Scope: `crates/shared/models/src/{artifacts,services,api}/`. 2026-05-15.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Shared-layer crate; only depends on `systemprompt_identifiers`, serde, schemars, chrono — no upward/cross deps. |
| 2 | Error model | clean | `api/errors/{mod,internal}.rs` use `thiserror`-derived enums; no `anyhow` anywhere in scope. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in any of the three directories. |
| 4 | Raw SQL | clean | No `sqlx::query` usage — pure DTO/model crate. |
| 5 | File size | remediated | `api/cloud/mod.rs` (336 lines) split into `tenant.rs`, `usage.rs`, `domain.rs`; `mod.rs` now 130 lines. All other files <300. |
| 6 | Function size | clean | No function exceeds the ~75-line guidance; builders are short. |
| 7 | Async traits | clean | `artifacts/traits.rs` traits are synchronous; no `async fn`/`#[async_trait]` in scope. |
| 8 | Typed identifiers | clean | Typed IDs used where modelled (`SkillId`, `ContextId`, `UserId`, `TenantId`, `PriceId`). Remaining `String` id fields are external-wire DTOs (cloud API JSON, artifact protocol payloads) — boundary types, not internal entity refs. `.into()` calls are all `impl Into<String>` builder params, not typed-ID call-site shortcuts. |
| 9 | Comment standard | clean | No paraphrasing `///`; the single `///` block in `services/mod.rs` is a substantive WHY (include-file `settings` rejection). No WHAT/narration inline comments. |
| 10 | No legacy | clean | No compat shims or dual paths. `pub type` aliases in `cloud/mod.rs` are the live public API surface, not deprecation stubs. |
| 11 | Naming | clean | No `*Manager`; types are DTOs/enums. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No extractable repeated logic; builder patterns are intentionally per-type. |
| 14 | CHANGELOG accuracy | clean | `CHANGELOG.md` entries verified against code; cloud-DTO split is internal file reorganisation with no public-API change, so no entry required. |

## Remediation summary

- `api/cloud/mod.rs` split into four files (`mod.rs` + `tenant.rs` + `usage.rs` + `domain.rs`). Pure file reorganisation: all types remain re-exported from `api::cloud` with identical paths and names. No behavioural or public-API change.
