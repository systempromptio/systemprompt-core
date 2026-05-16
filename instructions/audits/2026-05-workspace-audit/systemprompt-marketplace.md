# Audit: systemprompt-marketplace

Crate: `crates/domain/marketplace/` — per-user marketplace filtering for the bridge manifest.
Date: 2026-05-16. Scope: standards fixes and safe refactors only, no behavioural changes.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only on `systemprompt-models`, `systemprompt-identifiers`, `systemprompt-database` — all downward (shared/infra). |
| 2 | Error model | clean | `MarketplaceFilterError` is a `thiserror` enum; no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`. |
| 4 | Raw SQL | clean | Crate runs no SQL; `DbPool` is only passed through to factories. |
| 5 | File size | clean | Largest source file is 67 lines, well under the 300-line limit. |
| 6 | Function size | clean | All functions are small; largest is ~10 lines. |
| 7 | Async traits | clean | `#[async_trait]` on `MarketplaceFilter`, with the `Arc<dyn ...>` dispatch reason documented on the trait. |
| 8 | Typed identifiers | clean | `UserId` used throughout; no raw `String` IDs. |
| 9 | Comment standard | clean | `//!` head substantive; `///` blocks encode real invariants (filter contract, priority ordering, factory failure semantics) — none are name paraphrases. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `MarketplaceFilter`, `AllowAllFilter`, `MarketplaceFilterRegistration`; no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No repeated logic. |
| 14 | CHANGELOG accuracy | clean | 0.9.0–0.9.2 entries match the code (`HookEntry` field present, registration docs present). |

## Remediated

- **Item 9 (lib.rs `//!` accuracy)**: the crate-head doc claimed "Depends on `systemprompt-models` and `systemprompt-identifiers` only. No database" — contradicted by the `systemprompt-database` dependency and `DbPool` use in `registry.rs`. Updated the layer paragraph to list `systemprompt-database` and corrected the public-surface list to include `hooks`, `MarketplaceFilterRegistration`, and `discover_filters`.

## Verification

- `SQLX_OFFLINE=true cargo clippy -p systemprompt-marketplace --all-targets --all-features -- -D warnings`: clean.
- `SQLX_OFFLINE=true cargo doc -p systemprompt-marketplace --no-deps`: clean.
