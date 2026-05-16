# Audit: systemprompt-cli — Area 2 (`src/commands/cloud/`)

Scope: `crates/entry/cli/src/commands/cloud/**`. Entry binary crate — `anyhow` permitted, per-item `///` banned.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only depends on `systemprompt_cloud`, `systemprompt_identifiers`, `systemprompt_logging` and sibling cloud modules — no sideways/circular deps. |
| 2 | Error model | clean | `anyhow::Result` throughout, permitted in entry crate. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`todo!`/`unimplemented!` anywhere in scope. |
| 4 | Raw SQL | clean | No `sqlx::query*` calls; DB work delegated to `pg_dump`/`pg_restore` subprocesses and `systemprompt_cloud`. |
| 5 | File size | clean | Largest files are 297 lines (`profile/mod.rs`, `tenant/mod.rs`) — all under the 300-line limit. |
| 6 | Function size | clean | Functions are decomposed into cohesive sub-steps; no oversized functions observed. |
| 7 | Async traits | clean | No trait definitions / `#[async_trait]` in scope; plain `async fn` only. |
| 8 | Typed identifiers | remediated | `tenant/create/local.rs` passed `tenant.id.clone().into()` to `add_tenant(TenantId, ...)` — replaced with `TenantId::new(tenant.id.clone())`. `StoredTenant.id` is `String` (cross-crate `infra/cloud` type, out of scope). Other `.into()` calls are `PathBuf::from` / `String` field assignment, not typed-ID violations. |
| 9 | Comment standard | clean | No `///` rustdoc in scope (correct for entry crate). The only inline `//` block (`tenant/docker/container.rs`) is a legitimate WHY-comment on UUID format constraints. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | No `*Manager` types; helpers use plain function names, display via `CliService`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | Shared logic already factored into `*_steps.rs`/`builders.rs`/`helpers.rs` modules. |
| 14 | CHANGELOG | n/a | Observations only; `CHANGELOG.md` not touched. |

## Summary
13 items clean, 1 remediated (typed-identifier call site in `tenant/create/local.rs`).
