# systemprompt-traits

[![Crates.io](https://img.shields.io/crates/v/systemprompt-traits.svg?style=flat-square)](https://crates.io/crates/systemprompt-traits)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-traits?style=flat-square)](https://docs.rs/systemprompt-traits)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Defines shared interfaces for runtime capabilities, provider integration, events and persistence consumers.

**Layer**: Shared. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace. Depends only on [`systemprompt-identifiers`](https://crates.io/crates/systemprompt-identifiers) and [`systemprompt-provider-contracts`](https://crates.io/crates/systemprompt-provider-contracts).

## Installation

```toml
[dependencies]
systemprompt-traits = "0.50"
```

Enable the `web` feature to pull in the `axum`-backed `ApiModule` trait:

```toml
[dependencies]
systemprompt-traits = { version = "0.50", features = ["web"] }
```

## Example

```rust
use async_trait::async_trait;
use systemprompt_traits::Service;

struct HealthPinger;

#[async_trait]
impl Service for HealthPinger {
    fn name(&self) -> &str { "health-pinger" }
    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
    async fn health_check(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> { Ok(true) }
}
```

## Module map

| Module | Contents |
|--------|----------|
| `ai_providers` | `AiSessionProvider`, `AiFilePersistenceProvider`, image-generation metadata, and storage config. |
| `analytics` | `AnalyticsProvider`, `FingerprintProvider`, session inputs, and `AnalyticsProviderError`. |
| `auth` | `UserProvider`, `RoleProvider`, `AuthUser`, `FederatedIdentityClaims`, and `AuthProviderError`. |
| `content` | `ContentProvider`, `ContentSummary`, `ContentItem`, and `ContentFilter`. |
| `context` | `AppContext`, `ConfigProvider`, `DatabaseHandle`, `ModuleRegistry`, `Module`, `ContextPropagation`, and the optional `ApiModule` (`web` feature). |
| `context_provider` | `ContextProvider` for conversation contexts and `ContextWithStats`. |
| `domain_config` | `DomainConfig` and `DomainConfigRegistry` for per-domain config loading. |
| `events` | Log, user, and analytics event publisher traits. |
| `extension_error` | `ExtensionError` trait, `ApiError`, and `McpErrorData` wire types. |
| `jwt` | `JwtValidationProvider`, `AgentJwtClaims`, and `GenerateTokenParams`. |
| `log_service` | Generic `LogService` persistence trait. |
| `module` | `register_module!` macro for compile-time module registration via `inventory`. |
| `registry` | `AgentRegistryProvider`, `McpRegistryProvider`, `AgentInfo`, and `McpServerInfo`. |
| `repository` | `RepositoryError` enum shared by domain repositories. |
| `scheduler` | `JobStatus` and scheduler error types. |
| `service` | `Service` and `AsyncService` lifecycle traits. |
| `startup_events` | Startup phase, service, and module event types plus publisher extensions. |
| `storage` | `FileStorage`, `StoredFileId`, and `StoredFileMetadata`. |
| `validation` | `Validate`, `MetadataValidation`, and the field-level `ValidationError`. |
| `validation_report` | `ValidationReport`, `StartupValidationReport`, and warning types. |

Provider traits re-exported from [`systemprompt-provider-contracts`](https://crates.io/crates/systemprompt-provider-contracts) (chat, LLM, tools, jobs) are surfaced at the crate root so downstream callers do not need to depend on both crates.

## Error model

Each provider trait pairs with a `thiserror`-derived error enum (for example `AnalyticsProviderError`, `AuthProviderError`, `JwtProviderError`, `FileStorageError`, `ContextPropagationError`, `DomainConfigError`). The cross-cutting `ExtensionError` trait is implemented by downstream errors so the API and MCP transports render them uniformly into `ApiError` (HTTP) or `McpErrorData` (MCP).

`RepositoryError` is the standard error type for repository implementations across domain crates and is `#[non_exhaustive]`.

## Async traits

Most provider traits are exposed as `Arc<dyn TraitName>` via the `Dyn*` aliases. Until trait dispatch supports native `async fn` on `dyn` traits, these continue to rely on `#[async_trait]`; each trait whose contract requires it is annotated with the rationale.

## Feature flags

| Feature | Default | Effect |
|---------|---------|--------|
| `web` | off | Enables the `ApiModule` trait and pulls in `axum` for HTTP routing. |

## Dependencies

- `systemprompt-provider-contracts`, `systemprompt-identifiers`: sibling shared-layer crates.
- `async-trait`: async methods on dyn-compatible traits.
- `thiserror`: error enum derives.
- `inventory`: compile-time module registration.
- `serde`, `serde_json`, `chrono`, `uuid`, `futures`, `http`, `tracing`, `xxhash-rust`.
- `axum` (optional, `web` feature).

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
