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
- Resolves the admitted composition into the `Kernel` and its extension state (`HostKernel` boots a composed binary from the profile).
- Validates system configuration and extensions before the server accepts traffic.
- Coordinates domain services without implementing business logic.

**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

## Modules

| Module | Purpose |
|--------|---------|
| `context` | `AppContext` and its planes (`DataPlane`, `ConfigPlane`, `Plugins`, `Subsystems`), plus the GeoIP and content-config loaders |
| `builder` | `AppContextBuilder::new(boot, admitted)` construction (`with_authz_hook`, `with_services`, …), plane assembly and kernel state seeding (`KernelSeed`, `kernel_state`) |
| `kernel`, `host_kernel` | `Kernel` resolution of the admitted members' provided state and slots (`KernelError`), and `HostKernel`, the host boot path |
| `startup_validation` | `StartupValidator` across files, rate limits, web/content config, agents, MCP servers, AI providers, and extensions |
| `database_context` | `DatabaseContext`, a database-only context for CLI tools that do not need the full runtime |
| `validation` | Runtime prerequisite checks (`validate_system`, `validate_database_path`) |
| `error` | `RuntimeError` / `RuntimeResult`, the typed error model for construction and validation |

`AppContext` is assembled either through `AppContextBuilder` (the bootstrap path: the `BootContext`, database init, the admitted composition, optional GeoIP and content config, tracing with database persistence) or directly through `AppContext::from_parts(data, cfg, plugins, subsystems)` for tests and embedders that own plane construction.

## Usage

```toml
[dependencies]
systemprompt-runtime = "0.87"
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database pool and migrations |
| `systemprompt-config` | Configuration loading |
| `systemprompt-models` | Module and config definitions |
| `systemprompt-logging` | Tracing and CLI output |
| `systemprompt-extension` | The admitted composition and its contributions |
| `systemprompt-analytics` | Analytics service and GeoIP |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
