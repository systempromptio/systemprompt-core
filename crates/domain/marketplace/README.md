# systemprompt-marketplace

[![Crates.io](https://img.shields.io/crates/v/systemprompt-marketplace.svg?style=flat-square)](https://crates.io/crates/systemprompt-marketplace)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-marketplace?style=flat-square)](https://docs.rs/systemprompt-marketplace)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Loads marketplace catalogs, resolves enabled marketplace membership, applies per-user access rules and assembles signed bridge manifests and plugin bundles.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Usage

```toml
[dependencies]
systemprompt-marketplace = "0.52"
```

```rust,ignore
use systemprompt_marketplace::register_marketplace_filter;

register_marketplace_filter!(MyAclFilter::new, priority = 100);
```

## Public Surface

| Item | Description |
|------|-------------|
| `MarketplaceService` | Read-only resolution over a borrowed `ServicesConfig`: lookup, default fallback, active marketplace, referential-integrity check. |
| `ManifestService` / `CanonicalView` | Assemble a scoped, filtered `MarketplaceCandidate` and sign the canonical view. |
| `catalog::CatalogContent` / `plugin_bundles` | On-disk loaders projecting the services tree into the signed `*Entry` records; `plugin_bundles` is the single source of the active, content-gated plugin bundles shared by the manifest and serving paths. |
| `bundle` (`build_plugin_bundle`, `PluginBundle`, `BundleContent`, `BundleFile`) | Build-from-spec assembler that owns the `.claude-plugin` bundle contract. |
| `scope_to_marketplace` / `active_marketplace` | Marketplace scoping of the catalogue lists. |
| `view::render_marketplace_json` / `render_marketplace_list` | JSON projections for the HTTP catalogue endpoints. |
| `MarketplaceFilter` | Async trait implemented by ACL backends, applied before signing. |
| `MarketplaceCandidate` | Mutable bundle of `plugins`, `skills`, `agents`, `hooks`, `managed_mcp_servers`, and `artifacts` vectors plus optional `marketplace_id` and `access`, handed to the filter. |
| `AllowAllFilter` | Passthrough default returned when no extension registers a filter. |
| `MarketplaceError` / `MarketplaceFilterError` | Crate-wide error (lookup, catalogue load, signing) and the narrower filter error folded into it. |
| `MarketplaceFilterRegistration` / `discover_filters` | `inventory`-collected registration record with priority ordering, and its lookup (highest priority first). |
| `register_marketplace_filter!` | Compile-time registration macro for a filter factory. |

## Wiring

`AppContext` holds an `Arc<dyn MarketplaceFilter>`. At startup the runtime calls `discover_filters()`, picks the highest-priority registration, and falls back to `AllowAllFilter` when no registration is present or the factory returns an error. The factory receives a `&DbPool`.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Manifest entry types (`PluginEntry`, `SkillEntry`, `AgentEntry`, `HookEntry`, `ManagedMcpServer`). |
| `systemprompt-identifiers` | Typed identifiers. |
| `systemprompt-database` | `DbPool` passed to filter factories. |
| `systemprompt-security` | Ed25519 signing of the canonical manifest view. |
| `async-trait` | Async methods on the `dyn`-compatible `MarketplaceFilter`. |
| `inventory` | Compile-time filter registration. |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

Git source synchronization and dependency verification use HTTPS with source-scoped Bearer credentials resolved by the runtime `GitSourceOrchestrator`. The Git subprocess clears ambient environment/configuration and credential helpers, disables redirects and submodule recursion, and enforces 60-second subprocess and 120-second aggregate Git deadlines. Output is limited to 8 MiB, retained trees to 256 files/8 MiB, and temporary repository storage is monitored against 64 MiB. Temporary directories are private on Unix and removed before a result is returned; reader threads are joined and the Git child is reaped after process-group termination. Native source execution on non-Unix servers is unavailable until process-tree cleanup is verified.

Dependency verification compares every retained revision with its registered source, exact snapshot commit, relative root, bytes and executable modes. Cycles, missing or extra revisions, incorrect bindings, submodules and nested Git metadata prevent attestation. Local-authored root candidates require an immutable administrative source binding through `POST /api/v1/sources/{id}/verification-bindings` before committed bytes can be verified; their local snapshot is preserved. Imported dependencies continue to require their exact retained Git commit. Complete immutable manifests are retained separately from historical single-resource evidence; publication of an improvement requires this complete manifest. `POST /api/v1/source-verifications` accepts a `DependencyVerificationRequest`; identical evidence returns the retained manifest, whose ID resolves through `GET /api/v1/source-verifications/{id}`. Administrative authentication and existing cookie-origin checks apply.

The test fixtures separate actual local TLS Git transport acceptance (authentication, rotation, redirect refusal and redacted errors) from injected authenticated dependency-tree fixtures (retained provenance, independent credentials, exact bytes/modes and manifest retry). Local TLS transport tests do not relax production outbound URL restrictions or establish acceptance against an external private Git provider.
