# systemprompt-runtime

[![Crates.io](https://img.shields.io/crates/v/systemprompt-runtime.svg?style=flat-square)](https://crates.io/crates/systemprompt-runtime)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-runtime?style=flat-square)](https://docs.rs/systemprompt-runtime)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Constructs and validates `AppContext`, including database pools, configuration, registries, authorization and analytics handles used by the runtime.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate:

- Builds and holds `AppContext`, the central runtime state container.
- Registers module routes at compile time through `inventory` macros.
- Validates system configuration and extensions before the server accepts traffic.
- Coordinates domain services without implementing business logic.

**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

## Modules

| Module | Purpose |
|--------|---------|
| `context` | `AppContext` and its planes (`DataPlane`, `ConfigPlane`, `Plugins`, `Subsystems`), plus the GeoIP and content-config loaders |
| `builder` | `AppContextBuilder` fluent construction (`with_extensions`, `with_marketplace_filter`) and plane assembly |
| `registry` | Compile-time module registration and routing (`ModuleApiRegistry`, `ModuleApiRegistration`, `ModuleType`, `WellKnownRoute`) |
| `startup_validation` | `StartupValidator` across files, rate limits, web/content config, agents, MCP servers, AI providers, and extensions |
| `database_context` | `DatabaseContext`, a database-only context for CLI tools that do not need the full runtime |
| `span` | `create_request_span`, builds a tracing span with user, session, trace, and context IDs |
| `wellknown` | `.well-known` endpoint metadata registry (`WellKnownMetadata`, `get_wellknown_metadata`) |
| `validation` | Runtime prerequisite checks (`validate_system`, `validate_database_path`) |
| `error` | `RuntimeError` / `RuntimeResult`, the typed error model for construction and validation |

`AppContext` is assembled either through `AppContextBuilder` (the bootstrap path: `ProfileBootstrap`, database init, extension discovery, optional GeoIP and content config, tracing with database persistence) or directly through `AppContext::from_parts(data, cfg, plugins, subsystems)` for tests and embedders that own plane construction.

## Usage

```toml
[dependencies]
systemprompt-runtime = "0.50"
```

### Macros

| Macro | Purpose |
|-------|---------|
| `register_module_api!` | Register module routes with the runtime registry |
| `register_wellknown_route!` | Register `.well-known` endpoints with optional metadata |

```rust
use systemprompt_runtime::{register_module_api, ServiceCategory, ModuleType};

register_module_api!(
    "my-module",
    ServiceCategory::Core,
    my_module::routes,
    true,
    ModuleType::Regular
);
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database pool and migrations |
| `systemprompt-config` | Configuration loading |
| `systemprompt-models` | Module and config definitions |
| `systemprompt-logging` | Tracing and CLI output |
| `systemprompt-extension` | Extension discovery and validation |
| `systemprompt-analytics` | Analytics service and GeoIP |
| `inventory` | Compile-time static registration |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
