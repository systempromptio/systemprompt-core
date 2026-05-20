# Cloud Infrastructure & Deployment

**Crate:** `crates/infra/cloud/` | **Package:** `systemprompt-cloud`

---

## Overview

The cloud infrastructure provides systemprompt.io Cloud integration:
- Tenant management (local and cloud)
- API client for cloud operations
- Checkout flow for subscription provisioning
- Credentials storage and bootstrap
- Docker image building and deployment

---

## Tenant Types

| Type | Description | Database |
|------|-------------|----------|
| `Local` | Self-hosted PostgreSQL | User-provided connection string |
| `Cloud` | Fly.io-managed instance | Auto-provisioned via checkout |

---

## CLI Commands

### Authentication

```bash
systemprompt cloud auth login       # OAuth login to systemprompt.io Cloud
systemprompt cloud auth logout      # Clear saved credentials
systemprompt cloud auth whoami      # Show current user and token status
```

### Tenant Management

```bash
systemprompt cloud tenant create    # Create new tenant (interactive)
systemprompt cloud tenant list      # List all configured tenants
systemprompt cloud tenant show [ID] # Show tenant details
systemprompt cloud tenant delete    # Delete a tenant
systemprompt cloud tenant edit      # Edit tenant configuration
systemprompt cloud tenant rotate-credentials  # Rotate DB credentials
```

### Deployment

```bash
systemprompt cloud deploy           # Build, push, and deploy to cloud
systemprompt cloud sync down        # Sync from cloud to local
systemprompt cloud sync up          # Sync from local to cloud
```

---

## Storage Locations

| File | Purpose |
|------|---------|
| `.systemprompt/credentials.json` | API token and endpoint |
| `.systemprompt/tenants.json` | Tenant store (per-project) |
| `.systemprompt/profiles/<id>/profile.yaml` | Tenant profile configuration |
| `.systemprompt/profiles/<id>/secrets.json` | Tenant secrets (DB URL, JWT, API keys) |

---

## Authentication Model

The control plane and tenant deployments live in separate trust domains
and authenticate independently.

### Operator JWT (control plane → operator)

The control plane signs operator JWTs with an RS256 key. The CLI obtains
this JWT through `systemprompt cloud auth login` (OAuth) and stores it
in `.systemprompt/credentials.json`. The JWT carries the operator's
identity, the tenants they are authorised to act on, and an expiry.

The control plane publishes the matching public key at
`{control_plane_url}/.well-known/jwks.json`.

### Tenant access token (tenant deployment → operator)

Tenant deployments do not trust the operator JWT directly. Each
deployment:

1. Registers `{control_plane_url}` as a `TrustedIssuer`.
2. Resolves signing keys from the issuer's JWKS endpoint.
3. Accepts operator JWTs only as the `subject_token` of an RFC 8693
   token-exchange request, never as a bearer on a resource endpoint.

The tenant audience claim on the issued access token is the
deployment's `tenant_id`.

### Token exchange (every tenant API call)

Every tenant API call is gated by a short-lived access token obtained
via RFC 8693 token-exchange against the tenant's own
`/api/v1/core/oauth/token` endpoint:

```
POST {tenant_api_url}/api/v1/core/oauth/token
Content-Type: application/x-www-form-urlencoded

grant_type=urn:ietf:params:oauth:grant-type:token-exchange
subject_token=<operator_jwt>
subject_token_type=urn:ietf:params:oauth:token-type:access_token
resource={tenant_api_url}
```

The deployment validates the `subject_token` against the trusted issuer's
JWKS, mints a tenant-scoped access token, and returns it as
`{access_token, token_type: "Bearer", expires_in, ...}`.

`CloudApiClient` caches the access token until 30 seconds before its
advertised expiry, and re-exchanges exactly once after a 401 before
propagating the error.

---

## Two-Phase Provisioning Flow

Cloud tenant creation uses a two-phase process:

### Phase 1: Infrastructure (Backend via Paddle webhook)

1. User completes Paddle checkout
2. Management API receives webhook
3. Creates Fly.io app, volume, IPs, certificate
4. Generates secrets (jwt_secret, database_url)
5. **NO machine created yet** - returns `status: "awaiting_deploy"`

### Phase 2: Deploy (CLI-driven)

1. CLI builds Docker image: `registry.fly.io/systemprompt-images:tenant-{id}`
2. Gets registry token from Management API
3. Pushes image to shared registry
4. Calls deploy endpoint (validates exact image match)
5. Management API creates machine with image
6. Returns `status: "running"` with URL

```
User -> Paddle -> Webhook -> Phase 1 (infra) -> CLI -> Phase 2 (deploy) -> Running
```

---

## Docker Image Contents

The production Docker image includes:

| Source | Destination | Contents |
|--------|-------------|----------|
| `target/release/systemprompt` | `/app/bin/` | Rust binary |
| `core/web/dist/` | `/app/web/` | React frontend |
| `services/` | `/app/services/` | Configuration (agents, skills, content) |
| `extensions/mcp/` | `/app/extensions/mcp/` | MCP servers |
| `.systemprompt/profiles/` | `/app/services/profiles/` | Environment profiles |

**Not in image** (injected at runtime):
- `DATABASE_URL`, `JWT_SECRET` - Generated by provisioner
- `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` - User-configured via Fly secrets

---

## Registry Security Model

All tenant images use a shared Fly.io registry (`systemprompt-images`):

- **Tenant-scoped tags**: `tenant-{uuid}` format
- **Exact validation**: Deploy rejects any image not matching expected tag
- **Empty registry app**: No machines, secrets, or volumes to compromise

---

## API Endpoints Reference

### Token Exchange

| Operation | Method | Endpoint |
|-----------|--------|----------|
| Exchange operator JWT for tenant access token | POST | `/api/v1/core/oauth/token` |

### Tenant Operations

| Operation | Method | Endpoint |
|-----------|--------|----------|
| List tenants | GET | `/api/v1/tenants` |
| Get tenant | GET | `/api/v1/tenants/{id}` |
| Update tenant | PATCH | `/api/v1/tenants/{id}` |
| Delete tenant | DELETE | `/api/v1/tenants/{id}` |
| Get status | GET | `/api/v1/tenants/{id}/status` |
| Stream events | GET | `/api/v1/tenants/{id}/events` (SSE) |
| Deploy | POST | `/api/v1/tenants/{id}/deploy` |
| Get registry token | GET | `/api/v1/tenants/{id}/registry-token` |
| Rotate credentials | POST | `/api/v1/tenants/{id}/rotate-credentials` |

### Secrets

| Operation | Method | Endpoint |
|-----------|--------|----------|
| Get secrets (one-time) | GET | `/api/v1/tenants/{id}/credentials/{token}` |
| List secrets | GET | `/api/v1/tenants/{id}/secrets` |
| Set secret | POST | `/api/v1/tenants/{id}/secrets` |
| Delete secret | DELETE | `/api/v1/tenants/{id}/secrets/{key}` |

### Checkout

| Operation | Method | Endpoint |
|-----------|--------|----------|
| Create checkout | POST | `/api/v1/checkout` |
| List plans | GET | `/api/v1/checkout/plans` |

---

## Environment Variables (Runtime)

Injected into Fly.io machines by the provisioner:

### Core
```
HOST=0.0.0.0
PORT=8080
RUST_LOG=info
SYSTEMPROMPT_SERVICES_PATH=/app/services
WEB_DIR=/app/web
```

### Database
```
DATABASE_URL=postgresql://user:pass@host:5432/dbname
DATABASE_TYPE=postgres
```

### Security
```
JWT_SECRET=<64-char-random>
JWT_ISSUER=systemprompt
```

### AI Provider Keys (optional)
```
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GEMINI_API_KEY=AIza...
```

---

## Data Structures

### StoredTenant

```rust
pub struct StoredTenant {
    pub id: String,
    pub name: String,
    pub app_id: Option<String>,      // Fly.io app ID (cloud only)
    pub hostname: Option<String>,    // Public hostname (cloud only)
    pub region: Option<String>,
    pub database_url: Option<String>,
    pub tenant_type: TenantType,     // Local or Cloud
}
```

### TenantStore

```rust
pub struct TenantStore {
    pub tenants: Vec<StoredTenant>,
    pub synced_at: DateTime<Utc>,
}
```

---

## Key Exports

```rust
pub use systemprompt_cloud::{
    CloudApiClient,
    CloudCredentials, CredentialsBootstrap,
    StoredTenant, TenantStore, TenantType,
    run_checkout_callback_flow,
};
```

---

## Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| Machine won't start | Memory too low | Scale to 512MB via API |
| Health check failing | App crash on startup | Check logs |
| Database connection error | Wrong DATABASE_URL | Rotate credentials |
| Certificate not working | DNS not propagated | Wait 5-10 minutes |
| Deploy stuck | Registry auth expired | Re-run `cloud auth login` |
| API keys not working | Secrets not set | Set via tenant secrets API |
| Tenant 401 on every call | Operator JWT expired or issuer not trusted | Re-run `cloud auth login`; verify deployment's `TrustedIssuer` matches the control-plane URL |
