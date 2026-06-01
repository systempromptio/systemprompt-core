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

Per-user marketplace filtering for systemprompt.io. Defines the `MarketplaceFilter` trait that the API's `GET /v1/bridge/manifest` handler invokes to decide which plugins, skills, agents, hooks, and managed MCP servers a given user is permitted to see. The filter runs before the manifest is signed, so the Ed25519 signature covers exactly the set the user is authorised for.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Usage

```toml
[dependencies]
systemprompt-marketplace = "0.13.1"
```

```rust,ignore
use systemprompt_marketplace::register_marketplace_filter;

register_marketplace_filter!(MyAclFilter::new, priority = 100);
```

## Public Surface

| Item | Description |
|------|-------------|
| `MarketplaceFilter` | Async trait implemented by ACL backends. |
| `MarketplaceCandidate` | Mutable bundle of `PluginEntry`, `SkillEntry`, `AgentEntry`, `HookEntry`, and `ManagedMcpServer` vectors handed to the filter. |
| `AllowAllFilter` | Passthrough default returned when no extension registers a filter. |
| `MarketplaceFilterError` | Error enum (`Backend`, `UnknownUser`, `Policy`). |
| `MarketplaceFilterRegistration` | `inventory`-collected registration record with priority ordering. |
| `discover_filters` | Returns registered filter factories, highest priority first. |
| `register_marketplace_filter!` | Compile-time registration macro for a filter factory. |

## Wiring

`AppContext` holds an `Arc<dyn MarketplaceFilter>`. At startup the runtime calls `discover_filters()`, picks the highest-priority registration, and falls back to `AllowAllFilter` when no registration is present or the factory returns an error. The factory receives a `&DbPool`.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Manifest entry types (`PluginEntry`, `SkillEntry`, `AgentEntry`, `HookEntry`, `ManagedMcpServer`). |
| `systemprompt-identifiers` | Typed identifiers. |
| `systemprompt-database` | `DbPool` passed to filter factories. |
| `async-trait` | Async methods on the `dyn`-compatible `MarketplaceFilter`. |
| `inventory` | Compile-time filter registration. |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-marketplace)** · **[docs.rs](https://docs.rs/systemprompt-marketplace)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
