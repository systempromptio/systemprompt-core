# Facade Feature Flags Reference

Complete feature matrix for the `systemprompt` facade crate. Every flag below is defined in `systemprompt/Cargo.toml` under `[features]`. Features are additive: enabling a flag pulls in the listed dependency crates and any features it implies.

The facade re-exports the workspace crates behind feature gates so a consumer depends on one crate and selects only the layers it needs.

## Usage

```toml
[dependencies]
systemprompt = { version = "0.14", features = ["api"] }
```

The default feature set is `core`. To take only a non-default layer, disable defaults:

```toml
[dependencies]
systemprompt = { version = "0.14", default-features = false, features = ["database"] }
```

## Flag matrix

| Flag | Pulls in (crates) | Implies (features) | Enables |
|------|-------------------|--------------------|---------|
| `default` | — | `core` | Default build. |
| `core` | `systemprompt-traits`, `systemprompt-models`, `systemprompt-identifiers`, `systemprompt-extension`, `systemprompt-template-provider` | — | Core traits, data models, typed identifiers, the extension framework, template provider traits. |
| `database` | `systemprompt-database`, `sqlx` | — | SQLx database abstraction and the `DbPool`. |
| `config` | `systemprompt-config` | — | Profile/secrets configuration loaders (bootstrap sequence). |
| `mcp` | `rmcp` | — | MCP (Model Context Protocol) support via the `rmcp` crate, including `rmcp-macros`. |
| `api` | `systemprompt-api`, `systemprompt-runtime`, `axum` | `core`, `database` | HTTP server, `AppContext`, runtime lifecycle. `systemprompt-runtime` is built with its `geolocation` feature. |
| `sync` | `systemprompt-sync` | — | Cloud synchronization. |
| `cloud` | `systemprompt-cloud` | — | Cloud API client, credentials, OAuth. |
| `logging` | `systemprompt-logging` | — | Tracing setup. |
| `loader` | `systemprompt-loader` | — | File/module discovery. |
| `events` | `systemprompt-events` | — | Event bus and SSE. |
| `client` | `systemprompt-client` | — | HTTP API client. |
| `security` | `systemprompt-security` | — | JWT, auth, manifest signing. |
| `test-utils` | — | `cloud` | Credential fixtures and other test helpers. |
| `cli` | `systemprompt-cli` | — | CLI entry point for product binaries (standalone). |
| `runtime` | `systemprompt-extension` | `cli` | Runtime builder for embedding systemprompt as a library. |
| `full` | all domain crates + `systemprompt-files`, `systemprompt-generator` (with `image-processing`), `systemprompt-scheduler` | `api`, `mcp`, `sync`, `cloud`, `cli`, `logging`, `config`, `loader`, `events`, `client`, `security`, plus `systemprompt-logging/cli` | Everything: all domain modules, all infrastructure layers, and the CLI. |

The crate names map to the workspace layers in `crates/`: shared (`traits`, `models`, `identifiers`, `extension`, `template-provider`), infra (`database`, `config`, `cloud`, `logging`, `loader`, `events`, `client`, `security`), domain (`agent`, `ai`, `mcp`, `oauth`, `users`, `content`, `analytics`, `marketplace`), app (`runtime`, `scheduler`, `generator`, `sync`), entry (`api`, `cli`).

## Inter-flag dependencies

These implications are encoded directly in `[features]`; enabling the left brings in the right automatically.

| Flag | Transitively enables |
|------|----------------------|
| `default` | `core` |
| `api` | `core`, `database` (and therefore `systemprompt-traits`, `systemprompt-models`, `systemprompt-identifiers`, `systemprompt-extension`, `systemprompt-template-provider`, `systemprompt-database`, `sqlx`) |
| `runtime` | `cli` |
| `test-utils` | `cloud` |
| `full` | `api`, `mcp`, `sync`, `cloud`, `cli`, `logging`, `config`, `loader`, `events`, `client`, `security` (and everything those imply) |

`full` is the only flag that aggregates the domain crates (`agent`, `ai`, `mcp`, `oauth`, `users`, `content`, `analytics`, `marketplace`, `scheduler`, `generator`, `files`). There is no narrower flag that selects an individual domain crate through the facade.

## `docs.rs`

The crate is documented on docs.rs with `all-features = true` and the `docsrs` cfg (`systemprompt/Cargo.toml:151`), so the published documentation reflects the `full` surface.

## Bundled examples

Each example under `systemprompt/examples/` declares the feature it needs (`required-features`):

| Example | Required feature |
|---------|------------------|
| `extension` | `core` |
| `database` | `database` |
| `api` | `api` |
| `cli` | `cli` |

## Selecting a flag set

| Goal | Feature set |
|------|-------------|
| Author a compile-time extension (traits, models, identifiers) | `core` (default) |
| Query the database directly | `database` |
| Load a `profile.yaml` and secrets | `config` |
| Embed the HTTP server and `AppContext` | `api` |
| Build a standalone CLI binary | `cli` |
| Embed the runtime builder as a library | `runtime` |
| Everything (product binary) | `full` |

## Related

- [HTTP API reference](./http-api.md) — the routes the `api` feature mounts.
- [Extensions](../concepts/extensions.md) — the framework the `core` feature provides.
- [Profile configuration](./configuration.md) — the schema the `config` feature loads.
