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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The client that ships your deployment to systemprompt.io Cloud without handing over ownership. It logs you in, provisions the tenant, renders the Dockerfile, and packs your secrets, then hands you a running instance you control.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

Rent-a-dashboard AI keeps your prompts, keys, and audit trail on someone else's servers. This crate is the opposite path. It authenticates against the systemprompt.io control plane, provisions tenants, and produces the deployment image for a binary that runs on your infrastructure with your secrets injected at boot.

It is the seam between the local CLI or runtime and the control plane. Every credential, session, and tenant record lives on disk under XDG-aware paths so the same machine can hold several tenants at once.

## Modules

| Module | Purpose |
|--------|---------|
| `api_client` | Bearer-authenticated REST client for the Cloud API, with SSE streams for provisioning and checkout events. |
| `auth` | JWT expiry decoding for stored session tokens. |
| `checkout` | Paddle checkout callback flow and the provisioning watcher (`wait_for_provisioning`). |
| `cli_session` | Multi-tenant CLI sessions: `CliSession`, `SessionStore`, and the `Local` / `Tenant` session key. |
| `constants` | Production and sandbox API URLs and other fixed endpoints. |
| `credentials` | On-disk credential storage, validation, and file operations. |
| `credentials_bootstrap` | Process-wide credential initialization with its own error type. |
| `deploy` | Dockerfile rendering (`DockerfileBuilder`) and deployment-image validation. |
| `docker` | Docker invocations behind a `CommandRunner` seam (`DockerCli`). |
| `error` | `CloudError` / `CloudResult` and recovery-hint message helpers. |
| `oauth` | Browser-driven OAuth login against GitHub and Google via a local callback server. |
| `paths` | XDG-aware discovery of credentials, sessions, tenants, and project files. |
| `profile_authoring` | Pure `Profile` construction for local and cloud deployment targets. |
| `secrets_env` | Deploy-time mapping of `secrets.json` to environment variables, including the signing-key PEM transport encoding. |
| `tenants` | Persistent tenants index (`TenantStore`, `StoredTenant`, `TenantType`). |

## Usage

```toml
[dependencies]
systemprompt-cloud = "0.21"
```

```rust
use systemprompt_cloud::{CloudApiClient, CloudCredentials, Environment};

async fn whoami() -> Result<(), Box<dyn std::error::Error>> {
    let creds = CloudCredentials::load()?;
    let client = CloudApiClient::new(Environment::Production, creds);
    let me = client.user_me().await?;
    println!("Logged in as {}", me.user.email);
    Ok(())
}
```

## Public API

All fallible APIs return `CloudResult<T>` (`Result<T, CloudError>`). `CloudError` composes `reqwest`, `std::io`, `serde_json`, and `CredentialsBootstrapError` via `#[from]`.

| Item | Purpose |
|------|---------|
| `CloudApiClient` | Bearer-token REST client; typed methods for user, tenant, checkout, and deploy calls. |
| `CloudCredentials`, `CredentialsBootstrap` | On-disk and process-wide cloud credentials. |
| `CliSession`, `SessionStore`, `SessionKey` | Multi-tenant CLI session storage. |
| `TenantStore`, `StoredTenant`, `TenantType` | Persistent local tenants index. |
| `run_oauth_flow`, `run_checkout_callback_flow` | Browser-driven OAuth and Paddle checkout flows. |
| `wait_for_provisioning` | SSE plus polling watcher for tenant provisioning state. |
| `CloudPaths`, `resolve_path`, `expand_home`, `get_cloud_paths` | XDG-aware path resolution for credentials, tenants, profiles, and projects. |
| `DockerfileBuilder` | Renders the deployment Dockerfile for the owned binary. |
| `DockerCli`, `CommandRunner` | Docker invocations behind a testable command seam. |
| `Environment`, `OAuthProvider` | Production / Sandbox target and GitHub / Google login provider. |

Response types re-exported from `api_client` include `UserMeResponse`, `Tenant`, `TenantInfo`, `TenantStatus`, `TenantSecrets`, `Plan`, `CheckoutResponse`, `CheckoutEvent`, `ProvisioningEvent`, `DeployResponse`, and `RegistryToken`.

## Dependencies

- `systemprompt-models` — profile and module models
- `systemprompt-identifiers` — typed identifiers
- `systemprompt-loader` — services-config and profile discovery
- `systemprompt-extension` — extension registration surface
- `systemprompt-logging` — CLI service output (`cli` feature)
- `reqwest`, `reqwest-eventsource`, `async-stream`, `futures` — HTTP and SSE
- `axum` — local callback server
- `serde`, `serde_json`, `chrono` — serialization
- `clap`, `open`, `urlencoding`, `base64` — CLI and encoding utilities
- `thiserror`, `tokio`, `tracing` — errors, async runtime, logging

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-cloud)** · **[docs.rs](https://docs.rs/systemprompt-cloud)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
