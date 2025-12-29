# systemprompt-cloud

Cloud infrastructure services for SystemPrompt including API client, credentials, OAuth, and tenant management.

## Structure

```
cloud/
├── Cargo.toml
└── src/
    ├── lib.rs                    # Crate root, public exports
    ├── api_client.rs             # CloudApiClient for API communication
    ├── constants.rs              # OAuth, checkout, credential constants
    ├── context.rs                # CloudContext, ResolvedTenant
    ├── credentials.rs            # CloudCredentials management
    ├── credentials_bootstrap.rs  # CredentialsBootstrap initialization
    ├── error.rs                  # CloudError, CloudResult
    ├── jwt.rs                    # JWT token handling
    ├── paths.rs                  # CloudPath, ProjectPath, ProjectContext
    ├── tenants.rs                # TenantStore, StoredTenant
    ├── checkout/
    │   ├── mod.rs                # Checkout exports
    │   └── client.rs             # Checkout callback flow
    └── oauth/
        ├── mod.rs                # OAuth exports
        └── client.rs             # OAuth flow handling
```

## Public API

### Types

| Type | Source | Description |
|------|--------|-------------|
| `CloudApiClient` | `api_client.rs` | HTTP client for SystemPrompt Cloud API |
| `CloudCredentials` | `credentials.rs` | API token and authentication data |
| `CredentialsBootstrap` | `credentials_bootstrap.rs` | Global credential initialization |
| `CloudContext` | `context.rs` | Resolved cloud context |
| `ResolvedTenant` | `context.rs` | Resolved tenant information |
| `TenantStore` | `tenants.rs` | Local tenant cache storage |
| `StoredTenant` | `tenants.rs` | Cached tenant data |
| `CloudError` | `error.rs` | Cloud operation errors |
| `Environment` | `lib.rs` | Production or Sandbox environment |
| `OAuthProvider` | `lib.rs` | GitHub or Google OAuth |
| `CloudPath` | `paths.rs` | User-scoped paths (credentials, tenants) |
| `ProjectPath` | `paths.rs` | Project-scoped paths |
| `ProfilePath` | `paths.rs` | Profile-relative paths |
| `ProjectContext` | `paths.rs` | Project path resolution context |
| `CloudPaths` | `paths.rs` | Resolved cloud paths |

### API Client Types

| Type | Source | Description |
|------|--------|-------------|
| `UserMeResponse` | `api_client.rs` | Current user information |
| `Tenant` | `api_client.rs` | Tenant metadata |
| `TenantInfo` | `api_client.rs` | Detailed tenant info |
| `TenantStatus` | `api_client.rs` | Tenant provisioning status |
| `TenantSecrets` | `api_client.rs` | Tenant secrets (JWT, database URL) |
| `Plan` | `api_client.rs` | Subscription plan |
| `CheckoutResponse` | `api_client.rs` | Paddle checkout result |
| `DeployResponse` | `api_client.rs` | Deployment result |
| `RegistryToken` | `api_client.rs` | Container registry credentials |

### Functions

| Function | Source | Description |
|----------|--------|-------------|
| `run_oauth_flow` | `oauth/client.rs` | Execute OAuth authentication flow |
| `run_checkout_callback_flow` | `checkout/client.rs` | Handle Paddle checkout callback |
| `resolve_path` | `paths.rs` | Resolve path relative to base directory |
| `get_cloud_paths` | `paths.rs` | Get cloud paths from profile |

### Templates

| Type | Source | Description |
|------|--------|-------------|
| `OAuthTemplates` | `oauth/client.rs` | HTML templates for OAuth callbacks |
| `CheckoutTemplates` | `checkout/client.rs` | HTML templates for checkout callbacks |

## Dependencies

- `systemprompt-models` - Profile and module models
- `systemprompt-core-logging` - CLI service output
- `reqwest` - HTTP client
- `axum` - Callback server
- `serde` / `serde_json` - Serialization
- `chrono` - Timestamps
- `anyhow` / `thiserror` - Error handling
- `tokio` - Async runtime
- `tracing` - Logging
- `clap` - CLI argument parsing
