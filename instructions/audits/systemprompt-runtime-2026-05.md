# systemprompt-runtime Tech Debt Audit

**Layer:** app
**Audited:** 2026-05-04 (Wave D1 sweep)
**Verdict:** CLEAN

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments on every `pub` item | yes |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in public signatures | 0 |
| `async_trait` references | 0 |

**Total scored violations:** 0

---

## Architectural Compliance

Layer: `app`. Per `instructions/information/boundaries.md` dependencies
flow downward only. No circular imports. The crate composes typed
errors from `systemprompt-config`, `systemprompt-database`,
`systemprompt-files`, `systemprompt-users`, `systemprompt-analytics`,
`systemprompt-extension`, and `systemprompt-models` via `#[from]`.

---

## Public-API Hygiene

| Check | Status |
|-------|--------|
| Typed error boundary (`RuntimeError` / `RuntimeResult`) | PASS |
| `///` rustdoc on every `pub` item | PASS |
| `//!` rustdoc on every `pub mod` (effectively all modules; private ones too) | PASS |
| `//!` feature-flag matrix in `lib.rs` | PASS |
| `[package.metadata.docs.rs] all-features = true` | PASS |
| `cargo doc -D warnings` | PASS |
| `cargo clippy --all-targets --all-features -D warnings` | PASS |
| `just check-bans-crate systemprompt-runtime` | PASS |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 17 |
| Files over 300 lines | 0 |
| Largest file | `crates/app/runtime/src/startup_validation/mod.rs` (258 LOC) |
| Public items (pub fn/struct/enum/trait/type/const/use) | ~71 |

### File splits (Wave D1)

- `context.rs` 326 -> 214 LOC. Loaders extracted into
  `context_loaders.rs`. Trait impls (`AppContextTrait`,
  `ExtensionContext`, `HasAnalytics`, `HasFingerprint`,
  `HasUserService`, `HasRouteClassifier`) moved to
  `context_traits.rs`.

---

## Typed-error boundary

`RuntimeError` (in `src/error.rs`) composes upstream typed errors:

```text
RuntimeError
├── Profile(ConfigError)             // systemprompt_config (composite)
├── ProfileBootstrap(ProfileBootstrapError)
├── Config(ConfigError)              // systemprompt_models (singleton accessor)
├── Paths(PathError)                 // systemprompt_models
├── Files(FilesError)                // systemprompt_files
├── Users(UserError)                 // systemprompt_users
├── Repository(RepositoryError)      // systemprompt_database
├── Analytics(AnalyticsError)        // systemprompt_analytics
├── Loader(LoaderError)              // systemprompt_extension
├── EmptyDatabaseUrl
├── DatabaseNotFound { path }
├── DatabaseNotFile  { path }
└── Other(anyhow::Error)             // absorbs upstream still-anyhow APIs
```

`Other(#[from] anyhow::Error)` exists because some upstream
infrastructure (database installation, connectivity probes) still
returns `anyhow::Result`. It is the only remaining surface for
`anyhow::Error` in the crate and is invisible to callers using
typed pattern matching.

---

## Verdict

**CLEAN**

Wave D1 outcome:

- 5 anyhow public signatures -> 0
- 1 file >300 lines -> 0
- 5 `#[allow(...)]` attributes -> 0
- Every `pub` item now carries `///` rustdoc; every module file has
  `//!` rustdoc; `lib.rs` advertises the `geolocation` feature flag in
  a Markdown table; `[package.metadata.docs.rs] all-features = true`
  is set so docs.rs builds the full surface.
- `cargo build --all-features`, `cargo clippy --all-targets
  --all-features -D warnings`, `RUSTDOCFLAGS=-D warnings cargo doc
  --all-features --no-deps`, and `just check-bans-crate
  systemprompt-runtime` all pass.
