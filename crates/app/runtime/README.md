<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://systemprompt.io/documentation">Documentation</a> • <a href="https://github.com/systempromptio/systemprompt-core">Core</a> • <a href="https://github.com/systempromptio/systemprompt-template">Template</a></p>
</div>

---


# systemprompt-runtime

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../../assets/readme/terminals/dark/app-runtime.svg">
    <source media="(prefers-color-scheme: light)" srcset="../../../assets/readme/terminals/light/app-runtime.svg">
    <img alt="systemprompt-runtime terminal demo" src="../../../assets/readme/terminals/dark/app-runtime.svg" width="100%">
  </picture>
</div>

Application runtime context and module registry for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-runtime.svg)](https://crates.io/crates/systemprompt-runtime)
[![Documentation](https://docs.rs/systemprompt-runtime/badge.svg)](https://docs.rs/systemprompt-runtime)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

## Overview

**Part of the App layer in the systemprompt.io architecture.**
**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

Provides centralized access to database connections, configuration, extension services, and startup validation.

This crate is the application-layer orchestrator that:

- Initializes and manages the `AppContext` - the central runtime state container
- Provides compile-time module registration via `inventory` macros
- Validates system configuration and extensions at startup
- Coordinates domain services without implementing business logic

## File Structure

```
src/
├── lib.rs                    # Public exports and registration macros
├── context.rs                # AppContext builder and runtime state
├── database_context.rs       # Standalone database context for CLI tools
├── installation.rs           # Module schema and seed installation
├── registry.rs               # Module API registry and routing
├── span.rs                   # Request tracing span construction
├── startup_validation/       # Multi-domain configuration validation
│   ├── mod.rs                # StartupValidator orchestration
│   ├── config_loaders.rs     # Config file loading utilities
│   ├── display.rs            # Validation report rendering
│   ├── extension_validator.rs # Extension validation logic
│   ├── files_validator.rs    # FilesConfig domain validator
│   └── mcp_validator.rs      # MCP manifest validation
├── validation.rs             # Runtime system checks
└── wellknown.rs              # Well-known endpoint metadata registry
```

## Modules

### `context.rs`

**Purpose:** Central runtime state container providing access to all shared resources.

| Export | Description |
|--------|-------------|
| `AppContext` | Holds database pool, config, registries, analytics, and GeoIP reader |
| `AppContextBuilder` | Fluent builder for customized context initialization |

Key behaviors:
- Loads configuration via `ProfileBootstrap` and `Config::get()`
- Initializes database connection from config
- Discovers and validates extensions via `ExtensionRegistry`
- Optionally loads GeoIP database and content configuration
- Initializes tracing with database persistence

### `database_context.rs`

**Purpose:** Lightweight database-only context for CLI tools that don't need full runtime.

| Export | Description |
|--------|-------------|
| `DatabaseContext` | Minimal context with just a database pool |

### `installation.rs`

**Purpose:** Module installation orchestration for schema and seed data.

| Export | Description |
|--------|-------------|
| `install_module` | Installs a module using a new AppContext |
| `install_module_with_db` | Installs a module with an existing database connection |

Delegates to `systemprompt-database` for actual SQL execution.

### `registry.rs`

**Purpose:** Compile-time module registration and runtime routing.

| Export | Description |
|--------|-------------|
| `ModuleApiRegistry` | Collects all registered module routes at startup |
| `ModuleApiRegistration` | Static registration struct submitted via `inventory` |
| `ModuleRuntime` | Trait for modules to expose their routes |
| `WellKnownRoute` | Registration for `.well-known` endpoints |

### `span.rs`

**Purpose:** Constructs tracing spans from request context.

| Export | Description |
|--------|-------------|
| `create_request_span` | Builds a `RequestSpan` with user, session, trace, and context IDs |

### `startup_validation/`

**Purpose:** Multi-domain configuration validation at application startup.

| Export | Description |
|--------|-------------|
| `StartupValidator` | Orchestrates validation across all domain config validators |
| `display_validation_report` | Renders validation errors to console |
| `display_validation_warnings` | Renders validation warnings to console |

Submodules:
- `config_loaders.rs` - YAML config loading with spinner feedback
- `display.rs` - Console rendering for validation reports
- `extension_validator.rs` - Extension config and asset validation
- `files_validator.rs` - `FilesConfigValidator` domain implementation
- `mcp_validator.rs` - MCP server manifest validation

Validates: files, rate limits, web config, content config, agents, MCP servers, AI providers, and extensions.

### `validation.rs`

**Purpose:** Runtime system prerequisite checks.

| Export | Description |
|--------|-------------|
| `validate_system` | Validates database connection and path |

### `wellknown.rs`

**Purpose:** Metadata registry for `.well-known` endpoints.

| Export | Description |
|--------|-------------|
| `WellKnownMetadata` | Static metadata (path, name, description) for discovery |
| `get_wellknown_metadata` | Retrieves metadata for a given path |

## Macros

| Macro | Purpose |
|-------|---------|
| `register_module_api!` | Register module routes with the runtime registry |
| `register_wellknown_route!` | Register `.well-known` endpoints with optional metadata |

### Usage

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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-runtime = "0.0.1"
```

## License

Business Source License 1.1 - See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE) for details.
