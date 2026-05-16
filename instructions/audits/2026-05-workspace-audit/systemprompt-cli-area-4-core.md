# Audit: systemprompt-cli — area 4, `src/commands/core/`

Scope: `crates/entry/cli/src/commands/core/**`. Entry binary crate (`anyhow` permitted, `///` banned).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only downward deps (agent/content/files/database/runtime/identifiers/logging/models). |
| 2 | Error model | clean | `anyhow::Result` throughout — permitted in entry crate. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`todo!`; `unwrap_or_else` fallbacks only. |
| 4 | Raw SQL | clean | No `sqlx::query*` in scope; all DB access via repository types. |
| 5 | File size | clean | Largest file `files/types.rs` at 254 lines; all under 300. |
| 6 | Function size | remediated | `artifacts/show.rs::execute_with_pool` (131 lines) split into `render_artifact`/`render_part`. `content/edit.rs::execute_with_pool` (77) left — already delegates to `apply_*` helpers, splitting further would be padding. |
| 7 | Async traits | clean | No `#[async_trait]`; no trait defs in scope. |
| 8 | Typed identifiers | clean | `FileId`/`ContentId`/`CampaignId`/`SourceId`/`ArtifactId` constructed via `::new()`. `.into()` calls are on `impl Into<String>` builder params and enum conversions, not typed IDs. |
| 9 | Comment standard | clean | No `///` rustdoc; no narrative `//` comments. |
| 10 | No legacy | clean | No shims/dual paths/stubs/dead code/`#[allow]`. |
| 11 | Naming | clean | No `*Manager`; repository/service types correctly named. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No notable copy-paste; part-rendering logic now factored into `render_part`. |
| 14 | CHANGELOG | clean | Not edited (observations only). |

## Remediation summary

- `artifacts/show.rs`: extracted display block from the 131-line `execute_with_pool` into
  cohesive private fns `render_artifact` and `render_part`; added `Artifact` import.
  No behavioural change.

Verification: `SQLX_OFFLINE=true cargo clippy -p systemprompt-cli --all-targets --all-features -- -D warnings`
and `cargo doc -p systemprompt-cli --no-deps` both pass clean.
