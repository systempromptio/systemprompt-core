# Audit — systemprompt-users

Crate: `crates/domain/users/` · Audited 2026-05-15 · Workspace 0.10.1

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only downward on infra (`database`, `extension`) and shared (`models`, `traits`, `identifiers`, `provider-contracts`). |
| 2 | Error model | clean | `UserError` via `domain_error!` (`thiserror`); no `anyhow` in any public signature. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`. |
| 4 | Raw SQL | clean | All access via `query!`/`query_as!`/`query_scalar!`; no runtime `sqlx::query`. |
| 5 | File size | clean | Largest source file is 280 lines (`repository/user/operations.rs`); all under the 300 limit. |
| 6 | Function size | clean | All functions well under the 75-line guidance. |
| 7 | Async traits | clean | `#[async_trait]` only on `dyn`-compatible trait impls (`Job`, `UserProvider`, `RoleProvider`), all defined in upstream crates. |
| 8 | Typed identifiers | remediated | `From<User> for AuthUser` and `user_to_auth_user` rebuilt `UserId` via `UserId::new(user.id.to_string())`; replaced with direct move/clone of the already-typed `UserId`. `UserId::new(Uuid::new_v4().to_string())` in `operations.rs` is correct (`UserId` has no `generate` flag). |
| 9 | Comment standard | clean | `//!` head substantive on `lib.rs`; no `///` paraphrase noise; no narration comments. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `*Service` throughout; no `*Manager`. `UserProviderImpl` is a thin trait-impl wrapper, acceptable. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | Repeated SELECT projections are inherent to compile-time-checked `query_as!` macros; no extractable runtime logic. |
| 14 | CHANGELOG accuracy | remediated | Top entry was 0.9.2 while workspace is 0.10.1; added the 0.10.0 entry recording the `build.rs`-generated migration discovery. README usage snippet version pin updated 0.9.2 → 0.10.1. |

## Remediations applied

- `services/user/provider.rs`: `user_to_auth_user` now uses `user.id.clone()` instead of `UserId::new(user.id.to_string())`.
- `services/user_provider.rs`: `From<User> for AuthUser` now moves `user.id` directly and uses `User::is_active()` instead of the `Some("active")` magic-string comparison (behaviour-preserving — `UserStatus::Active.as_str()` is `"active"`).
- `CHANGELOG.md`: added `[0.10.0]` entry for the migration-file refactor.
- `README.md`: dependency version pin 0.9.2 → 0.10.1.

No public-signature or behavioural changes. No issues requiring escalation to other crates.
