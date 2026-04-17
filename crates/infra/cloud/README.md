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

# systemprompt-cloud

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-cloud — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-cloud.svg?style=flat-square)](https://crates.io/crates/systemprompt-cloud)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-cloud?style=flat-square)](https://docs.rs/systemprompt-cloud)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Cloud API client, credentials, OAuth, and tenant management for systemprompt.io AI governance deployments. Remote sync and multi-tenant orchestration for the MCP governance pipeline. Includes API client, credentials, OAuth, and tenant management.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Cloud infrastructure services including API client, credentials, OAuth, and tenant management.

## Architecture

```
cloud/
├── Cargo.toml
├── status.md
└── src/
    ├── lib.rs                      # Crate root, public exports, Environment, OAuthProvider enums
    ├── constants.rs                # OAuth, checkout, credential, path constants
    ├── context.rs                  # CloudContext, ResolvedTenant
    ├── credentials.rs              # CloudCredentials management
    ├── credentials_bootstrap.rs    # CredentialsBootstrap global initialization
    ├── error.rs                    # CloudError, CloudResult
    ├── tenants.rs                  # TenantStore, StoredTenant, TenantType
    │
    ├── api_client/
    │   ├── mod.rs                  # API client exports
    │   ├── client.rs               # CloudApiClient core HTTP methods
    │   ├── tenant_api.rs           # Tenant-specific API methods
    │   ├── streams.rs              # SSE streaming for provisioning/checkout events
    │   └── types.rs                # Re-exports from systemprompt-models
    │
    ├── auth/
    │   ├── mod.rs                  # Auth exports
    │   └── token.rs                # JWT token expiry decoding
    │
    ├── checkout/
    │   ├── mod.rs                  # Checkout exports
    │   ├── client.rs               # Checkout callback flow handling
    │   └── provisioning.rs         # Wait for provisioning (SSE + polling)
    │
    ├── cli_session/
    │   ├── mod.rs                  # Session exports, SessionKey enum
    │   ├── session.rs              # CliSession, CliSessionBuilder
    │   └── store.rs                # SessionStore (multi-session management)
    │
    ├── oauth/
    │   ├── mod.rs                  # OAuth exports
    │   └── client.rs               # OAuth flow with local callback server
    │
    └── paths/
        ├── mod.rs                  # Path resolution exports
        ├── cloud.rs                # CloudPath, CloudPaths (credential/tenant paths)
        ├── context.rs              # UnifiedContext (combined path resolution)
        ├── discovery.rs            # DiscoveredProject (project root discovery)
        └── project.rs              # ProjectPath, ProfilePath, ProjectContext
```

### Module Overview

| Module | Purpose |
|--------|---------|
| `api_client` | HTTP client for systemprompt.io Cloud API with SSE streaming support |
| `auth` | JWT token handling and expiry checking |
| `checkout` | Paddle checkout callback flow and provisioning wait logic |
| `cli_session` | CLI session management with multi-tenant support |
| `context` | Cloud context resolution combining credentials and profile |
| `credentials` | Credential storage, validation, and file operations |
| `credentials_bootstrap` | Global singleton for credential initialization |
| `error` | Domain-specific error types with recovery hints |
| `oauth` | OAuth authentication flow with local callback server |
| `paths` | Path resolution for credentials, tenants, profiles, and projects |
| `tenants` | Local tenant cache storage and management |

## Usage

```toml
[dependencies]
systemprompt-cloud = "0.2.1"
```

```rust
use systemprompt_cloud::{CloudApiClient, CloudCredentials, Environment};

async fn whoami() -> anyhow::Result<()> {
    let creds = CloudCredentials::load()?;
    let client = CloudApiClient::new(Environment::Production, creds);
    let me = client.user_me().await?;
    println!("Logged in as {}", me.user.email);
    Ok(())
}
```

## Public API

### Types

| Type | Source | Description |
|------|--------|-------------|
| `CloudApiClient` | `api_client/client.rs` | HTTP client for systemprompt.io Cloud API |
| `CloudCredentials` | `credentials.rs` | API token and authentication data |
| `CredentialsBootstrap` | `credentials_bootstrap.rs` | Global credential initialization |
| `CloudContext` | `context.rs` | Resolved cloud context with tenant info |
| `ResolvedTenant` | `context.rs` | Resolved tenant information |
| `CliSession` | `cli_session.rs` | CLI session with auth tokens |
| `SessionStore` | `cli_session.rs` | Multi-session storage |
| `SessionKey` | `cli_session.rs` | Session identifier (Local or Tenant) |
| `TenantStore` | `tenants.rs` | Local tenant cache storage |
| `StoredTenant` | `tenants.rs` | Cached tenant data |
| `TenantType` | `tenants.rs` | Local or Cloud tenant type |
| `CloudError` | `error.rs` | Cloud operation errors |
| `Environment` | `lib.rs` | Production or Sandbox environment |
| `OAuthProvider` | `lib.rs` | GitHub or Google OAuth |
| `CloudPath` | `paths/cloud.rs` | User-scoped paths (credentials, tenants) |
| `CloudPaths` | `paths/cloud.rs` | Resolved cloud paths |
| `ProjectPath` | `paths/project.rs` | Project-scoped paths |
| `ProfilePath` | `paths/project.rs` | Profile-relative paths |
| `ProjectContext` | `paths/project.rs` | Project path resolution context |
| `DiscoveredProject` | `paths/discovery.rs` | Discovered project root |
| `UnifiedContext` | `paths/context.rs` | Combined path resolution |

### API Client Types

| Type | Source | Description |
|------|--------|-------------|
| `UserMeResponse` | `api_client/types.rs` | Current user information |
| `Tenant` | `api_client/types.rs` | Tenant metadata |
| `TenantInfo` | `api_client/types.rs` | Detailed tenant info |
| `TenantStatus` | `api_client/types.rs` | Tenant provisioning status |
| `TenantSecrets` | `api_client/types.rs` | Tenant secrets (JWT, database URL) |
| `Plan` | `api_client/types.rs` | Subscription plan |
| `CheckoutResponse` | `api_client/types.rs` | Paddle checkout result |
| `CheckoutEvent` | `api_client/types.rs` | SSE checkout event |
| `ProvisioningEvent` | `api_client/types.rs` | SSE provisioning event |
| `DeployResponse` | `api_client/types.rs` | Deployment result |
| `RegistryToken` | `api_client/types.rs` | Container registry credentials |

### Functions

| Function | Source | Description |
|----------|--------|-------------|
| `run_oauth_flow` | `oauth/client.rs` | Execute OAuth authentication flow |
| `run_checkout_callback_flow` | `checkout/client.rs` | Handle Paddle checkout callback |
| `wait_for_provisioning` | `checkout/provisioning.rs` | Wait for tenant provisioning |
| `resolve_path` | `paths/mod.rs` | Resolve path relative to base directory |
| `expand_home` | `paths/mod.rs` | Expand ~ in paths |
| `get_cloud_paths` | `paths/cloud.rs` | Get cloud paths from profile |

### Templates

| Type | Source | Description |
|------|--------|-------------|
| `OAuthTemplates` | `oauth/client.rs` | HTML templates for OAuth callbacks |
| `CheckoutTemplates` | `checkout/client.rs` | HTML templates for checkout callbacks |

## Dependencies

- `systemprompt-models` - Profile and module models
- `systemprompt-identifiers` - Typed identifiers
- `systemprompt-logging` - CLI service output
- `reqwest` - HTTP client
- `reqwest-eventsource` - SSE client
- `axum` - Callback server
- `serde` / `serde_json` - Serialization
- `chrono` - Timestamps
- `anyhow` / `thiserror` - Error handling
- `tokio` - Async runtime
- `tracing` - Logging
- `clap` - CLI argument parsing
- `validator` - Struct validation

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-cloud)** · **[docs.rs](https://docs.rs/systemprompt-cloud)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
