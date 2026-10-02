# systemprompt-extension

[![Crates.io](https://img.shields.io/crates/v/systemprompt-extension.svg?style=flat-square)](https://crates.io/crates/systemprompt-extension)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-extension?style=flat-square)](https://docs.rs/systemprompt-extension)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Defines extension contracts and compile-time registration for routes, schemas, migrations, jobs and providers. Registered implementations execute within the host process.

**Layer**: Shared: foundational types and traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

An extension declares its schemas, API routes, scheduled jobs, providers, seeds, and assets through the `Extension` trait. Authors register each one with the `register_extension!` macro, which submits it to the [`inventory`](https://docs.rs/inventory) linker collector. At startup the runtime gathers every registration, validates declared dependencies (`Extension::dependencies()`) and reserved API paths, and merges the resulting wiring into the host binary. `Extension` is the single extension trait: `metadata()` is required and every other hook is a defaulted method.

## Module Map

| Module | Purpose |
|--------|---------|
| `asset` (private, re-exported) | `AssetDefinition`, `AssetDefinitionBuilder`, `AssetPaths`, `AssetType`. |
| `build` | Build-script helper (`emit_migrations`) that generates `Extension::migrations()` from `schema/migrations/*.sql`, paired with the `extension_migrations!` macro. |
| `capabilities` | The `Has*` capability traits (`HasAnalytics`, `HasFingerprint`, `HasRouteClassifier`, `HasUserService`). |
| `context` | `ExtensionContext` and `DynExtensionContext` handed to extensions during router resolution. |
| `cost` | `CostDirective`, `CostDirectiveError`, `TriggerPolicy`. |
| `error` | `LoaderError`, `ExtensionConfigError`. |
| `frame_options` | Per-route `X-Frame-Options` override (`FrameOptions`, `FrameOptionsOverride`, `stamp_frame_options`) honoured by the host security-headers middleware. |
| `gateway_guard` | `GatewayRequestGuard` and the `register_gateway_guard!` macro for gateway request guards. |
| `metadata` | `ExtensionMetadata`, `ExtensionRole`, `SchemaDefinition`. |
| `migration` | `Migration` value type for versioned extension migrations. |
| `purge` | `UserPurgeTable` and orphan-sweep registrations (`user_purge_tables!`, `orphan_sweeps!`). |
| `registry` | `ExtensionRegistry`, `ExtensionRegistration`, `RESERVED_PATHS`, discovery, queries, validation. |
| `router` | `ExtensionRouter`, `ExtensionRouterConfig`, `SiteAuthConfig`. |
| `runtime_config` | Process-level fallback injection of extensions when the `inventory` collector is stripped (for example by LTO). |
| `seed` | `Seed`: idempotent post-migration data fixtures applied on every boot, outside migration tracking. |
| `traits` (private, re-exported) | The `Extension` trait and `register_extension!` macro. |

## Usage

```toml
[dependencies]
systemprompt-extension = "0.62"
```

```rust
use systemprompt_extension::prelude::*;

#[derive(Default)]
struct MyExtension;

impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "my-extension",
            name: "My Extension",
            version: "0.1.0",
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
- `async-trait`: `dyn`-compatible async hooks.
- `serde` / `serde_json`: Metadata and configuration serialisation.
- `thiserror`: Typed error enums.
- `tracing`: Structured logging.
- `xxhash-rust`: Migration checksums.
- `systemprompt-provider-contracts`: Provider trait definitions re-exported from the prelude.
- `systemprompt-traits`: Core shared traits.
- `systemprompt-identifiers`: Typed identifiers.

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
