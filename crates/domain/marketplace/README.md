<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-marketplace

[![Crates.io](https://img.shields.io/crates/v/systemprompt-marketplace.svg?style=flat-square)](https://crates.io/crates/systemprompt-marketplace)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-marketplace?style=flat-square)](https://docs.rs/systemprompt-marketplace)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Every user gets exactly the catalogue they are entitled to, signed. This crate resolves the active marketplace, loads the on-disk catalogue, scopes and per-user filters it, then hands it to signing, so the Ed25519 signature on the bridge manifest covers precisely the set that user is authorised for.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Usage

```toml
[dependencies]
systemprompt-marketplace = "0.21"
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

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-marketplace)** · **[docs.rs](https://docs.rs/systemprompt-marketplace)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
