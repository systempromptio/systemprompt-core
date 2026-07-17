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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Every route, schema, job, and provider that runs in your governance engine is declared here, at compile time, and collected into one audited startup path. No dynamic plugin loading, no runtime surprises. What links into the binary is what runs.

**Layer**: Shared: foundational types and traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

An extension declares its schemas, API routes, scheduled jobs, providers, seeds, and assets through the `Extension` trait. Authors register each one with the `register_extension!` macro, which submits it to the [`inventory`](https://docs.rs/inventory) linker collector. At startup the runtime gathers every registration, validates the dependency graph, and merges the resulting wiring into the host binary. Dependency ordering is enforced at compile time through a typestate builder, so an extension that names a missing dependency fails to build rather than to boot.

## Module Map

| Module | Purpose |
|--------|---------|
| `any` | Type-erased wrappers (`AnyExtension`, `ApiExtensionWrapper`, `SchemaExtensionWrapper`). |
| `asset` | `AssetDefinition`, `AssetDefinitionBuilder`, `AssetPaths`, `AssetType`. |
| `build` | Build-script helper (`emit_migrations`) that generates `Extension::migrations()` from `schema/migrations/*.sql`, paired with the `extension_migrations!` macro. |
| `builder` | `ExtensionBuilder`: fluent builder enforcing dependency ordering via typestate. |
| `capabilities` | `CapabilityContext`, `FullContext`, and the `Has*` capability traits. |
| `context` | `ExtensionContext` and `DynExtensionContext` handed to extensions during router resolution. |
| `error` | `LoaderError`, `ConfigError`. |
| `frame_options` | Per-route `X-Frame-Options` override (`FrameOptions`, `stamp_frame_options`) honoured by the host security-headers middleware. |
| `hlist` | Heterogeneous-list machinery (`TypeList`, `Contains`, `Subset`, `NotSame`) backing the dependency typestate. |
| `metadata` | `ExtensionMetadata`, `ExtensionRole`, `SchemaDefinition`. |
| `migration` | `Migration` value type for versioned extension migrations. |
| `registry` | `ExtensionRegistry`, `ExtensionRegistration`, discovery, queries, validation. |
| `router` | `ExtensionRouter`, `ExtensionRouterConfig`, `SiteAuthConfig`. |
| `runtime_config` | Process-level fallback injection of extensions when the `inventory` collector is stripped (for example by LTO). |
| `seed` | `Seed`: idempotent post-migration data fixtures applied on every boot, outside migration tracking. |
| `traits` | The `Extension` trait and `register_extension!` macro. |
| `typed` | Compile-time-checked sub-traits: `SchemaExtensionTyped`, `ApiExtensionTyped`, `ConfigExtensionTyped`, `JobExtensionTyped`, `ProviderExtensionTyped`. |
| `typed_registry` | `TypedExtensionRegistry` and `RESERVED_PATHS`. |
| `types` | `Dependencies`, `DependencyList`, `ExtensionMeta`, `ExtensionType`, `NoDependencies`. |

## Usage

```toml
[dependencies]
systemprompt-extension = "0.21"
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

- `inventory`: Compile-time extension registration.
- `axum`: Router types for `ExtensionRouter` and the frame-options middleware.
- `reqwest`: HTTP client types exposed through capability traits.
- `serde` / `serde_json`: Metadata and configuration serialisation.
- `thiserror`: Typed error enums.
- `tracing`: Structured logging.
- `systemprompt-provider-contracts`: Provider trait definitions re-exported from the prelude.
- `systemprompt-traits` (with `web` feature): Core shared traits.

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-extension)** · **[docs.rs](https://docs.rs/systemprompt-extension)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
