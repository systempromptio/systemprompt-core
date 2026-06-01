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

# systemprompt-loader

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-loader — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-loader.svg?style=flat-square)](https://crates.io/crates/systemprompt-loader)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-loader?style=flat-square)](https://docs.rs/systemprompt-loader)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

File and module discovery infrastructure for systemprompt.io AI governance — manifests, schemas, and extension loading. Separates I/O from shared models in the MCP governance pipeline. Provides pure I/O operations for loading configuration files, profiles, secrets, extensions, and module definitions without any domain logic.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides pure I/O operations for loading configuration files, profiles, secrets, extensions, and module definitions without any domain logic.

## Architecture

The loader crate sits in the infrastructure layer and depends only on `systemprompt-models` (shared layer). It separates file I/O concerns from business logic, enabling:

- Testable file operations with clear boundaries
- Reusable loaders across different entry points (API, CLI)
- Consistent configuration parsing and validation

```
src/
├── lib.rs                       # Public API exports
├── error.rs                     # ConfigLoad / ConfigWrite / ExtensionLoad / ProfileLoad error types
├── config_loader/               # Services configuration loader
│   ├── mod.rs                   # ConfigLoader entry point
│   ├── includes.rs              # Recursive `includes:` resolution with cycle detection
│   ├── merge.rs                 # Deep-merge logic for included fragments
│   └── types.rs                 # Loader-internal types
├── config_writer.rs             # Agent configuration file writer
├── extension_loader/            # Extension manifest discovery and loading
│   ├── mod.rs                   # ExtensionLoader entry point
│   ├── manifest.rs              # manifest.yaml parsing
│   └── result.rs                # ExtensionValidationResult
├── extension_registry.rs        # Runtime extension binary registry
├── module_loader.rs             # `inventory`-driven extension aggregator
├── profile_loader.rs            # Profile YAML loader with validation
└── modules/
    └── mod.rs                   # Module collection aggregator
```

### Core Loaders

| Module | Purpose |
|--------|---------|
| `ProfileLoader` | Loads and validates profile YAML files from the profiles directory |
| `ConfigLoader` | Loads services configuration, merges includes, and validates strict schema |
| `ModuleLoader` | Thin wrapper over the `inventory`-driven `ExtensionRegistry`; discovers compiled-in extensions and collects their schemas |

### Extension Support

| Module | Purpose |
|--------|---------|
| `ExtensionLoader` | Discovers on-disk extensions by scanning for `manifest.yaml` files |
| `ExtensionRegistry` | Runtime registry mapping binary names to extension metadata |
| `ConfigWriter` | Creates, updates, and deletes agent configuration files |

### Module Aggregation

The `modules` module re-exports the compile-time extension registry. `ModuleLoader::discover_extensions` returns every `inventory`-registered `Extension`, and `ModuleLoader::collect_extension_schemas` flattens their `SchemaDefinition`s for schema installation.

## Usage

```toml
[dependencies]
systemprompt-loader = "0.14.0"
```

### Features

| Feature | Default | Purpose |
|---------|---------|---------|
| `expose-internals` | off | Exposes test-only entry points such as `ConfigLoader::load_from_content` to dependent crates outside `cfg(test)`. |

```rust
use systemprompt_loader::{
    ConfigLoader, ProfileLoader,
    ExtensionLoader, ExtensionRegistry, ModuleLoader,
};

let config = ConfigLoader::load()?;

let loader = ConfigLoader::for_active_profile()?;

let profile = ProfileLoader::load_and_validate(services_path, "development")?;

let extensions = ModuleLoader::discover_extensions()?;
let schemas = ModuleLoader::collect_extension_schemas()?;

let discovered = ExtensionLoader::discover(project_root);
```

## Dependencies

- `thiserror` — typed error variants
- `serde` / `serde_yaml` / `serde_json` — serialisation
- `tokio` — async runtime
- `tracing` — structured logging
- `systemprompt-config` — profile and config primitives
- `systemprompt-extension` — extension trait registry
- `systemprompt-identifiers` — typed IDs
- `systemprompt-models` — shared model types

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-loader)** · **[docs.rs](https://docs.rs/systemprompt-loader)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
