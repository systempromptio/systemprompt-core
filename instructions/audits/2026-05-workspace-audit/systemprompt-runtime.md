# Audit: systemprompt-runtime

Crate: `crates/app/runtime/` — application runtime (AppContext, builder, registries, startup validation).
Date: 2026-05-16. Result: clean across all 14 checklist items; one CHANGELOG accuracy fix applied.

1. **Layering** — clean. Deps flow downward only (app → domain analytics/files/users + infra database/config/security/logging/loader + shared models/traits/extension/identifiers). No upward or sibling-app deps.
2. **Error model** — clean. Typed `RuntimeError` (`thiserror`, `error.rs`) composes upstream errors via `#[from]`; no `anyhow` in any signature.
3. **No panics** — clean. No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`. The single `unwrap_or_else` in `config_loaders.rs` is an infallible fallback, not a panic.
4. **Raw SQL** — clean. No SQL in this crate; database access delegated to `systemprompt-database`.
5. **File size** — clean. Largest file `startup_validation/mod.rs` at 253 lines, under the 300-line limit.
6. **Function size** — clean. All functions within ~75-line guidance; `StartupValidator::validate` already decomposed into `load_configs` / `load_domain_validators` / `run_domain_validations`.
7. **Async traits** — clean. No custom async traits defined; `async fn` used only on inherent impls.
8. **Typed identifiers** — clean. No raw `String` entity IDs in struct fields or service args; `database_url` strings are connection URLs, not entity IDs.
9. **Comment standard** — clean. Substantive `//!` heads on `lib.rs` and module files; no `///` paraphrase walls; inline `//` only on the genuine WHY in `builder.rs::with_migrations`.
10. **No legacy** — clean. No shims, dual paths, or `Option<T>` migration stubs. The `installation` module noted as removed in CHANGELOG 0.9.2 is gone.
11. **Naming** — clean. `*Service`/`*Handler`/`*Validator`/`*Builder`/`*Registry`; no `*Manager`.
12. **Tests location** — clean. No inline `#[cfg(test)] mod tests`.
13. **Local duplication** — clean. The repeated extension-error-report pattern in `extension_validator.rs` is short and call-site-specific; not worth extracting.
14. **CHANGELOG accuracy** — remediated. The per-crate `CHANGELOG.md` stopped at 0.9.2 while the crate is at 0.10.2. Added a 0.10.2 entry recording the `ExtensionRegistry::discover()` → `Result` adaptation in `builder.rs` and `startup_validation/extension_validator.rs`.
