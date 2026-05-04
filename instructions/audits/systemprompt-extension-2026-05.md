# systemprompt-extension Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave A1 compliance sweep)
**Verdict:** CLEAN

---

## Summary (post-sweep)

| Category | Before | After |
|----------|--------|-------|
| `let _ =` discards | 3 (in default trait method bodies suppressing unused params) | 0 (replaced with `_param` naming) |
| `pub` items with rustdoc | ~0 | 192 (every public type, trait method, fn, const, macro) |
| Module-level `//!` docs | 0 | every `pub mod` + crate-level matrix |
| `anyhow::` references | 0 | 0 |
| `async_trait` references | 0 | 0 (`async-trait` dep removed from `Cargo.toml`; `anyhow` dep removed too) |
| `unwrap()`/`expect()` | 0 | 0 |
| `panic!()`/`todo!()`/`unimplemented!()` | 0 | 0 |
| `println!`/`eprintln!`/`dbg!` | 0 | 0 |
| Files >300 lines | 0 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `#[allow(...)]` | 0 | 0 |

---

## Fixes Applied

### Public errors

`error.rs` already exposed `LoaderError` and `ConfigError` `thiserror`
enums; both now carry full rustdoc on every variant and field.

### `let _ =` discards (3) eliminated

The three `let _ = <param>;` lines in default trait method bodies
(`Extension::router`, `Extension::validate_config`,
`ConfigExtensionTyped::validate_config`) suppressing unused params have
been replaced with `_param` naming, which is the idiomatic Rust form and
needs no `// Why:` annotation.

### File-cohesion split

`traits.rs` was 239 lines (under the 300 limit) but the rustdoc pass
pushed it past 300. Split into:

- `traits/mod.rs` — re-exports
- `traits/extension.rs` (281) — the `Extension` trait declaration
- `traits/register.rs` (29) — the `register_extension!` macro

### Rustdoc coverage

- Crate-level `//!` with authoring example and module map added to
  `lib.rs`.
- Every `pub mod` carries a `///` line at the declaration site.
- Every `pub use` re-export carries a `///` line describing the
  re-exported item.
- Every method on the `Extension` trait carries a `///` summary
  (≈40 methods across required fns and `has_*` predicates).
- Every public type (`AssetDefinition`, `AssetDefinitionBuilder`,
  `AssetType`, `AssetPaths`, `ExtensionRouter`, `ExtensionRouterConfig`,
  `SiteAuthConfig`, `Migration`, `ExtensionMetadata`,
  `SchemaDefinition`, `SchemaSource`, `SeedSource`, `ExtensionRole`,
  `WebAssetsStrategy`, `InjectedExtensions`, `CapabilityContext`,
  `ExtensionType`, `ExtensionMeta`, `Dependencies`, `DependencyList`,
  `NoDependencies`, `MissingDependency`, `TypeList`, `Subset`,
  `Contains`, `NotSame`, `AnyExtension`, `ExtensionWrapper`,
  `SchemaExtensionWrapper`, `ApiExtensionWrapper`, `ExtensionBuilder`,
  `TypedExtensionRegistry`, `ExtensionRegistry`,
  `ExtensionRegistration`, all `Has*` capability traits, all
  `*ExtensionTyped` traits, `SchemaDefinitionTyped`,
  `SchemaSourceTyped`) carries doc.
- The `register_extension!` macro carries usage examples.

### Dead-dep removal

`async-trait` and `anyhow` were declared in `Cargo.toml` but unused in
the source. Both removed.

### `[package.metadata.docs.rs]`

Added to `Cargo.toml` with `all-features = true`. (The crate has no
Cargo features today, but the block matches the workspace-wide
convention.)

---

## Verification

```
cargo fmt -p systemprompt-extension                                       PASS
cargo build -p systemprompt-extension --all-features                      PASS
cargo clippy -p systemprompt-extension --all-targets --all-features
    -- -D warnings                                                        PASS
RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-extension
    --no-deps --all-features                                              PASS
just check-bans                                                           PASS (0 violations from this crate)
```

---

## File Statistics (post-sweep)

| Metric | Value |
|--------|-------|
| Total .rs files | 27 |
| Files over 300 lines | 0 |
| Largest file | `traits/extension.rs` (281 lines) |

---

## Verdict

**CLEAN**
