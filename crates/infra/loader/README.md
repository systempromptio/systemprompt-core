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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Your deployment's configuration lives in files you own. This crate reads them, so no other layer has to know how the disk is laid out. It loads services config, profiles, and extension manifests, and writes agent files back.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

The loader isolates file I/O from the shared model types. It sits one level above `systemprompt-config` in the dependency graph, so domain crates read services config, profiles, and extensions without knowing the on-disk structure. That boundary keeps file operations testable and the loaders reusable across the API and CLI entry points.

## Modules

| Module | Purpose |
|--------|---------|
| `config_loader` | `ConfigLoader` reads `services.yaml`, resolves `includes:` recursively with cycle detection (`discovery.rs`, `includes.rs`), deep-merges fragments (`merge.rs`), and validates against a strict schema. |
| `config_writer` | `ConfigWriter` creates, edits, and deletes agent configuration files. |
| `extension_loader` | `ExtensionLoader` discovers on-disk extensions by scanning for `manifest.yaml`, returning an `ExtensionValidationResult`. |
| `extension_registry` | `ExtensionRegistry` maps binary names to extension metadata and resolves binary paths. |
| `module_loader` | `ModuleLoader` wraps the `inventory`-driven registry: `discover_extensions` returns every compiled-in `Extension`, `collect_extension_schemas` flattens their `SchemaDefinition`s. |
| `profile_loader` | `ProfileLoader` reads, validates, and writes profile YAML. |
| `error` | `ConfigLoadError`, `ConfigWriteError`, `ExtensionLoadError`, `ProfileLoadError` and their result aliases. |

## Usage

```toml
[dependencies]
systemprompt-loader = "0.21"
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
- `serde` / `serde_yaml` — serialisation
- `tracing` — structured logging
- `systemprompt-config` — profile and config primitives
- `systemprompt-extension` — extension trait registry
- `systemprompt-models` — shared model types

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-loader)** · **[docs.rs](https://docs.rs/systemprompt-loader)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
