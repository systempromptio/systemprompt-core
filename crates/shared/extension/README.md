# systemprompt-extension

[![Crates.io](https://img.shields.io/crates/v/systemprompt-extension.svg?style=flat-square)](https://crates.io/crates/systemprompt-extension)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-extension?style=flat-square)](https://docs.rs/systemprompt-extension)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Defines the composition contract for members: the `Extension` trait, the `Contributions` sink through which a member declares its routes, schemas, migrations, jobs, commands and capability slot entries, and the admitted handoffs the kernel reads. Admitted members execute within the host process.

**Layer**: Shared: foundational types and traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

A member implements `Extension`: `metadata()` and one `contribute(&mut Contributions)` that declares everything it brings — schemas, migrations, seeds, provided and required state, jobs, retention, projections, route sets (`RouteSet`), gateway routes, CLI commands (`CommandDecl`), readiness checks, diagnostics, metrics, gateway guards, site auth, configs, assets and capability `Slot` entries (`c.slot::<S>()`, returning the slot's builder). Every declaration only stores. An install names its members in its `Composition`; nothing is collected at link time. The composition admits the members against the profile before anything they declare is built, and the kernel re-checks every privileged entry against the constants in `reserved` (reserved API prefixes, built-in gateway prefixes, reserved CLI paths and privileged route shapes).

## Module Map

| Module | Purpose |
|--------|---------|
| `admitted`, `admitted_*` | The `Admitted`, `BuiltState` and `DeclaredCommands` handoffs, and the admitted assets, configs, migrations, purge plan and schedule. |
| `asset` (private, re-exported) | `AssetDefinition`, `AssetDefinitionBuilder`, `AssetPaths`, `AssetType`. |
| `authz`, `protection` | `AuthzPolicy`, `HandlerVerifier` and the protection a mounted route group carries. |
| `brain`, `notifications` | The seam traits the brain and notification slots collect. |
| `build` | Build-script helper (`emit_migrations`) that generates the migration list from `schema/migrations/*.sql`, read with the `extension_migrations!` macro. |
| `capabilities` | The `Has*` capability traits (`HasRouteClassifier`). |
| `cli` | `KernelGrant`; with the `cli` feature, `CliCommandGroup`, `CliCommandLeaf`, `CommandDecl`, `CommandHost`, `CommandOutput` and `CommandError`. |
| `contribute` | `Contributions`, `Resolved` and `FromState`, deferred `ProvideFn`s and `Factory`s, `Slot`s and kernel handles. |
| `cost` | `CostDirective`, `CostDirectiveError`, `TriggerPolicy`. |
| `error` | `LoaderError`, `ExtensionStateError`, `ExtensionConfigError`, `MigrationFault`, `SchemaFault`. |
| `frame_options` | Per-route `X-Frame-Options` override (`FrameOptionsOverride`, `stamp_frame_options`) over the provider-contracts `FrameOptions`, honoured by the host security-headers middleware. |
| `gateway_guard` | `GatewayRequestGuard` and `GatewayGuards`, the guards a gateway consults. |
| `metadata` | `ExtensionMetadata`, `ExtensionRole`, `SchemaDefinition`. |
| `migration` | `Migration` value type for versioned extension migrations. |
| `projection`, `retention` | Projection drains and retention declarations. |
| `purge` | `UserPurgeTable`, `PurgePredicate` and `OrphanSweep` contributions, and their owned `PurgeDeclaration` form. |
| `reserved` | `RESERVED_PATHS`, `BUILTIN_GATEWAY_PREFIXES`, `RESERVED_CLI_PATHS`, `PRIVILEGED_SHAPES`, `CoreId`. |
| `router` | Route bases, specs and shapes; with the `router` feature, `RouteSet` and the HTTP helpers capability handlers use. |
| `seed` | `Seed`: idempotent post-migration data fixtures applied on every boot, outside migration tracking. |
| `traits` (private, re-exported) | The `Extension` trait. |

## Usage

```toml
[dependencies]
systemprompt-extension = "0.86"
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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.migrations(extension_migrations!());
        Ok(())
    }
}

```

## Feature Flags

- `router`: axum route sets, the capability HTTP error, content negotiation and server-sent events.
- `cli`: clap command declarations and the command host.
- `slack`, `teams`: the Slack and Teams privileged shapes.
- `sqlx`: the brain and notification sinks.

## Dependencies

- `axum`, `tower`, `futures`, `http` (`router` feature): Route sets and the frame-options middleware.
- `clap` (`cli` feature): Command declarations.
- `pg_query`: Parsing schema SQL to find the tables a member creates.
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
