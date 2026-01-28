<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-loader

File loading infrastructure for systemprompt.io - separates I/O from shared models.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-loader.svg)](https://crates.io/crates/systemprompt-loader)
[![Documentation](https://docs.rs/systemprompt-loader/badge.svg)](https://docs.rs/systemprompt-loader)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Infra layer in the systemprompt.io architecture.**

This crate provides pure I/O operations for loading configuration files, profiles, secrets, extensions, and module definitions without any domain logic.

## Architecture

The loader crate sits in the infrastructure layer and depends only on `systemprompt-models` (shared layer). It separates file I/O concerns from business logic, enabling:

- Testable file operations with clear boundaries
- Reusable loaders across different entry points (API, TUI, CLI)
- Consistent configuration parsing and validation

## File Structure

```
src/
├── lib.rs                       # Public API exports
├── config_loader.rs             # Basic services configuration loader
├── config_writer.rs             # Agent configuration file writer
├── enhanced_config_loader.rs    # Extended config loader with includes
├── extension_loader.rs          # Extension manifest discovery and loading
├── extension_registry.rs        # Runtime extension binary registry
├── include_resolver.rs          # YAML !include directive resolver
├── module_loader.rs             # Module definition aggregator
├── profile_loader.rs            # Profile YAML loader with validation
└── modules/
    ├── mod.rs                   # Module collection aggregator
    ├── agent.rs                 # Agent module definition
    ├── ai.rs                    # AI service module definition
    ├── analytics.rs             # Analytics module definition
    ├── api.rs                   # API gateway module definition
    ├── content.rs               # Content management module definition
    ├── database.rs              # Database module definition
    ├── files.rs                 # File management module definition
    ├── log.rs                   # Logging module definition
    ├── mcp.rs                   # MCP service module definition
    ├── oauth.rs                 # OAuth module definition
    ├── scheduler.rs             # Scheduler module definition
    └── users.rs                 # Users module definition
```

## Module Overview

### Core Loaders

| Module | Purpose |
|--------|---------|
| `ProfileLoader` | Loads and validates profile YAML files from the profiles directory |
| `ConfigLoader` | Loads services configuration with include file merging |
| `EnhancedConfigLoader` | Extended config loader with agent discovery and include resolution |
| `ModuleLoader` | Aggregates all module definitions for database schema management |

### Extension Support

| Module | Purpose |
|--------|---------|
| `ExtensionLoader` | Discovers extensions by scanning for `manifest.yaml` files |
| `ExtensionRegistry` | Runtime registry mapping binary names to extension metadata |
| `IncludeResolver` | Resolves `!include` directives in YAML configuration |
| `ConfigWriter` | Creates, updates, and deletes agent configuration files |

### Module Definitions

The `modules/` directory contains definitions for each systemprompt.io module. Each definition specifies:

- Module metadata (name, version, description)
- Database schemas with required columns
- Seed data for initial setup
- API configuration
- Module dependencies and load order (weight)

## Usage

```rust
use systemprompt_loader::{
    ConfigLoader, ProfileLoader,
    ExtensionLoader, ExtensionRegistry, ModuleLoader,
};

let config = ConfigLoader::load()?;

let profile = ProfileLoader::load_and_validate(services_path, "development")?;

let modules = ModuleLoader::all();

let extensions = ExtensionLoader::discover(project_root);
```

## Dependencies

- `anyhow` - Error handling
- `thiserror` - Error type definitions
- `serde` / `serde_yaml` / `serde_json` - Serialization
- `tokio` - Async runtime
- `tracing` - Logging
- `systemprompt-models` - Shared model types

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-loader = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
