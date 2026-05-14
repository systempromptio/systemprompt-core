# systemprompt-marketplace

Per-user marketplace filtering for systemprompt.io.

Defines the `MarketplaceFilter` trait that the gateway's
`/v1/bridge/manifest` handler invokes to decide which plugins, skills,
agents, hooks, and managed MCP servers a given user is permitted to
see. The filter runs before the manifest is signed, so the Ed25519
signature covers exactly the set the user is authorised for.

## Public surface

- `MarketplaceFilter` — async trait implemented by ACL backends.
- `MarketplaceCandidate` — mutable bundle of `PluginEntry`,
  `SkillEntry`, `AgentEntry`, `HookEntry`, and `ManagedMcpServer`
  vectors handed to the filter.
- `AllowAllFilter` — passthrough default returned when no extension
  registers a filter.
- `MarketplaceFilterError` — `Backend`, `UnknownUser`, `Policy`.
- `MarketplaceFilterRegistration` and `register_marketplace_filter!` —
  `inventory`-based compile-time registration with priority ordering.

## Layer

Domain crate. Depends on `systemprompt-models`,
`systemprompt-identifiers`, and `systemprompt-database` (the factory
receives a `&DbPool`). No HTTP, no runtime, no extension framework
hooks beyond `async-trait` and `inventory`.

## Wiring

`AppContext` holds an `Arc<dyn MarketplaceFilter>`. At startup the
runtime calls `discover_filters()`, picks the highest-priority
registration, and falls back to `AllowAllFilter` when no registration
is present or the factory returns an error.

```rust,ignore
use systemprompt_marketplace::register_marketplace_filter;
register_marketplace_filter!(MyAclFilter::new, priority = 100);
```
