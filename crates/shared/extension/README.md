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

# systemprompt-extension

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-extension — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-extension.svg?style=flat-square)](https://crates.io/crates/systemprompt-extension)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-extension?style=flat-square)](https://docs.rs/systemprompt-extension)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Compile-time extension framework for systemprompt.io AI governance infrastructure. Built on the `inventory` crate — registers schemas, API routes, jobs, and providers in the MCP governance pipeline. Extensions can add new routes, services, and capabilities to the platform.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Provides the compile-time framework for building and loading systemprompt.io extensions. Extensions declare schemas, API routes, jobs, providers, and assets through the `Extension` trait, are registered via the `inventory` crate, and are collected by the host runtime at startup.

## Module Map

| Module | Purpose |
|--------|---------|
| `any` | Type-erased wrappers (`AnyExtension`, `ApiExtensionWrapper`, `SchemaExtensionWrapper`). |
| `asset` | `AssetDefinition`, `AssetDefinitionBuilder`, `AssetPaths`, `AssetType`. |
| `builder` | `ExtensionBuilder` — fluent builder enforcing dependency ordering via typestate. |
| `capabilities` | `CapabilityContext`, `FullContext`, and `Has*` capability traits. |
| `context` | `ExtensionContext` and `DynExtensionContext`. |
| `error` | `LoaderError`, `ConfigError`. |
| `hlist` | Heterogeneous list machinery (`TypeList`, `Contains`, `Subset`, `NotSame`). |
| `metadata` | `ExtensionMetadata`, `ExtensionRole`, `SchemaDefinition`. |
| `migration` | `Migration` value type for versioned extension migrations. |
| `registry` | `ExtensionRegistry`, `ExtensionRegistration`, discovery, queries, validation. |
| `router` | `ExtensionRouter`, `ExtensionRouterConfig`, `SiteAuthConfig`. |
| `runtime_config` | Runtime configuration surface for extensions. |
| `traits` | The `Extension` trait and `register_extension!` macro. |
| `typed` | Compile-time-checked sub-traits: `SchemaExtensionTyped`, `ApiExtensionTyped`, `ConfigExtensionTyped`, `JobExtensionTyped`, `ProviderExtensionTyped`. |
| `typed_registry` | `TypedExtensionRegistry` and `RESERVED_PATHS`. |
| `types` | `Dependencies`, `DependencyList`, `ExtensionMeta`, `ExtensionType`, `MissingDependency`, `NoDependencies`. |

## Usage

```toml
[dependencies]
systemprompt-extension = "0.13.0"
```

```rust
use systemprompt_extension::prelude::*;

#[derive(Default)]
struct MyExtension;

impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            name: "my-extension".into(),
            version: "0.1.0".into(),
            role: ExtensionRole::Domain,
            ..Default::default()
        }
    }
}

register_extension!(MyExtension);
```

## Feature Flags

None. This crate has no Cargo features; everything compiles into every build.

## Dependencies

- `inventory` — Compile-time extension registration.
- `axum` — Router types for `ExtensionRouter`.
- `reqwest` — HTTP client types exposed through capability traits.
- `serde` / `serde_json` — Metadata and configuration serialisation.
- `thiserror` — Typed error enums.
- `tracing` — Structured logging.
- `systemprompt-provider-contracts` — Provider trait definitions re-exported from the prelude.
- `systemprompt-traits` — Core shared traits (with `web` feature).

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-extension)** · **[docs.rs](https://docs.rs/systemprompt-extension)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
