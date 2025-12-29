# Cloud Infrastructure

**Crate:** `crates/infra/cloud/`
**Package:** `systemprompt-cloud`

---

## Overview

The cloud infrastructure crate provides SystemPrompt Cloud integration including:

- API client for cloud operations
- Tenant management (local and cloud)
- Checkout flow for subscription provisioning
- Credentials storage and bootstrap

---

## Tenant Types

SystemPrompt supports two tenant types:

| Type | Description | Database |
|------|-------------|----------|
| `Local` | Self-hosted PostgreSQL | User-provided connection string |
| `Cloud` | Fly.io-managed instance | Auto-provisioned via checkout |

---

## CLI Commands

### Tenant Management

```bash
# Create a new tenant (interactive wizard)
systemprompt cloud tenant create

# List all configured tenants
systemprompt cloud tenant list

# Show tenant details
systemprompt cloud tenant show [ID]

# Delete a tenant from local config
systemprompt cloud tenant delete [ID]
```

### Cloud Setup Flow

```bash
# Full setup wizard (login → checkout → tenant config)
systemprompt cloud setup

# Login to SystemPrompt Cloud
systemprompt cloud login

# Configure existing tenant
systemprompt cloud config
```

---

## Data Structures

### StoredTenant

Persisted to `.systemprompt/tenants.json`:

```rust
pub struct StoredTenant {
    pub id: String,              // Unique tenant identifier
    pub name: String,            // Display name
    pub app_id: Option<String>,  // Fly.io app ID (cloud only)
    pub hostname: Option<String>,// Public hostname (cloud only)
    pub region: Option<String>,  // Deployment region
    pub database_url: Option<String>, // PostgreSQL connection string
    pub tenant_type: TenantType, // Local or Cloud
}

pub enum TenantType {
    Local,  // User-managed PostgreSQL
    Cloud,  // Fly.io-managed instance
}
```

### TenantStore

Container for all tenants with sync metadata:

```rust
pub struct TenantStore {
    pub tenants: Vec<StoredTenant>,
    pub synced_at: DateTime<Utc>,
}
```

---

## Constructors

Use the appropriate constructor based on tenant type:

```rust
// Local tenant with PostgreSQL connection
StoredTenant::new_local(id, name, database_url)

// Cloud tenant from checkout flow
StoredTenant::new_cloud(id, name, app_id, hostname, region, database_url)

// From API response
StoredTenant::from_tenant_info(&tenant_info)
```

---

## Storage Locations

| File | Purpose |
|------|---------|
| `~/.systemprompt/credentials.json` | API token and endpoint |
| `.systemprompt/tenants.json` | Tenant store (per-project) |
| `~/.systemprompt/secrets.json` | Cloud secrets after checkout |
| `~/.systemprompt/cloud.secrets.profile.yml` | Generated profile |

---

## Checkout Flow

1. User selects plan and region
2. CLI creates checkout session via Cloud API
3. Browser opens Paddle checkout
4. On success, Paddle redirects to local callback server
5. CLI polls for tenant provisioning status
6. Secrets URL becomes available when ready
7. CLI fetches secrets and saves locally
8. Tenant added to store with database URL

---

## Cloud API Client

```rust
use systemprompt_cloud::{CloudApiClient, CloudCredentials};

let creds = CloudCredentials::load()?;
let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

// List user's tenants
let tenants = client.list_tenants().await?;

// Get tenant status
let status = client.get_tenant_status(&tenant_id).await?;

// Create checkout
let checkout = client.create_checkout(&price_id, &region, redirect_uri).await?;
```

---

## Key Exports

```rust
pub use crate::api_client::CloudApiClient;
pub use crate::credentials::{CloudCredentials, CredentialsBootstrap};
pub use crate::tenants::{StoredTenant, TenantStore, TenantType};
pub use crate::checkout::{run_checkout_callback_flow, CheckoutTemplates};
```
