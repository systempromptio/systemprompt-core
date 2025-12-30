# Cloud Deployment Architecture

This document details the complete flow from local development to a running cloud tenant, including what gets saved where and how code is deployed remotely.

## Overview

The deployment system spans three repositories:
- **systemprompt-template**: Local development environment and Docker packaging
- **systemprompt-core**: CLI tools for tenant management and deployment
- **systemprompt-db**: Management API with Fly.io provisioning

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TWO-PHASE PROVISIONING FLOW                               │
└─────────────────────────────────────────────────────────────────────────────┘

  DEVELOPER MACHINE                    MANAGEMENT API                FLY.IO
  ─────────────────                    ──────────────                ──────

  PRE-REQUISITE: BUILD FIRST
  ══════════════════════════
  $ just build --release
  ├── cargo build --release
  ├── npm run build (web)
  └── Verify: binary, web/dist, Dockerfile exist

  PHASE 1: INFRASTRUCTURE PROVISIONING
  ════════════════════════════════════
  $ just tenant (cloud)
  │
  │   CLI validates build exists ────► Error if not built
  │
  ├── POST /api/v1/checkout ─────────────────────────────────────────────────┐
  │                                      │                                   │
  │   Paddle Checkout ◄──────────────────┘                                   │
  │   (User pays)                                                            │
  │        │                                                                 │
  │        ▼                                                                 │
  │   Webhook ─────────────────────► POST /webhooks/paddle                   │
  │                                      │                                   │
  │                              Fly Provisioner ───────────────► Create App │
  │                                      │                        Create Vol │
  │                                      │                        Alloc IPs  │
  │                                      │                        Add Cert   │
  │                                      │  (NO machine yet!)                │
  │                                      ▼                                   │
  │   SSE: InfrastructureReady ◄─────────                                    │
  │   { status: "awaiting_deploy", app_id: "sp-xxx" }                        │
  │                                                                          │
  PHASE 2: BUILD & DEPLOY
  ═══════════════════════
  │   docker build -t registry.fly.io/systemprompt-images:tenant-{id}        │
  │                                      │                                   │
  ├── GET /tenants/{id}/registry-token ──┤                                   │
  │   { registry, token, repository: "systemprompt-images", tag: "tenant-{id}" }
  │                                      │                                   │
  │   docker push ───────────────► SHARED REGISTRY ─────────────────────────┤
  │                                registry.fly.io/systemprompt-images       │
  │                                      │                                   │
  ├── POST /tenants/{id}/deploy ─────────┤                                   │
  │   { image: "registry.fly.io/systemprompt-images:tenant-{id}" }           │
  │   │                                  │                                   │
  │   │  Validate: image == expected ────┤  (EXACT match required!)          │
  │                                      │                                   │
  │                              Create Machine ────────────────► Machine    │
  │                              Wait for Running                 Starts     │
  │                                      │                                   │
  │   SSE: TenantReady ◄─────────────────┘                                   │
  │   { status: "running", url: "https://..." }                              │
  │                                                                          │
  └── Save to .systemprompt/                                                 │
      ├── tenants.json                                                       │
      └── profiles/<id>/                                                     │
          ├── profile.yaml                                                   │
          └── secrets.json                                                   │

  SUBSEQUENT DEPLOYS
  ══════════════════
  $ just deploy
  │
  ├── docker build -t registry.fly.io/systemprompt-images:tenant-{id}
  ├── docker push ───────────────► registry.fly.io/systemprompt-images
  └── POST /tenants/{id}/deploy ─► Update Machine ────────────► New Image
```

## Key Concepts

### Shared Registry: systemprompt-images

All tenant Docker images are pushed to a single shared Fly.io registry app (`systemprompt-images`).
There is no per-tenant registry - all tenants share one registry with tenant-scoped tags.

### Build Before Provision

Cloud tenant creation requires a successful build first. The CLI validates:
- `target/release/systemprompt` exists
- `core/web/dist` exists
- `.systemprompt/Dockerfile` exists

### Two-Phase Provisioning

1. **Phase 1 (Backend)**: Creates infrastructure (app, volume, IPs, cert) but NO machine
2. **Phase 2 (CLI)**: Builds image, pushes to shared registry, triggers deploy to create machine

## Registry Security Model

### Shared Registry Architecture

All tenant images are pushed to a single shared Fly.io registry app:

```
systemprompt-images (registry-only app)
├── No machines     → Can't steal compute
├── No secrets      → Nothing to exfiltrate
├── No volumes      → No data to access
└── Deploy token    → Scoped to this empty app only
```

### Tenant-Scoped Image Tags

Images are tagged using the format `tenant-{tenant_id}`:
- Full image reference: `registry.fly.io/systemprompt-images:tenant-{uuid}`
- Tags are cryptographically strong (UUIDs)
- Each tenant can only push to their own tag

### Security Controls

| Layer | Control | Location |
|-------|---------|----------|
| API Authentication | User must be authenticated | AuthUser middleware |
| Tenant Ownership | User must own the tenant | `verify_ownership()` |
| Subscription Access | User's subscription must be active | `has_tenant_access()` |
| Registry Token | Org token, but tag is tenant-scoped | `get_registry_token()` handler |
| Image Validation | **Exact** image match required | `DeployRequest::validate_image()` |

### Image Validation on Deploy

The deploy endpoint performs strict validation:

```rust
// In handlers.rs:205-221
let expected_image = provisioner.config().build_full_image(&tenant_id);
// e.g., "registry.fly.io/systemprompt-images:tenant-abc123"

req.validate_image(&expected_image)?;  // Must match EXACTLY
```

This prevents:
- Tenant A deploying Tenant B's image
- Arbitrary image injection
- Tag manipulation attacks

### Registry Token Response

When tenants request registry credentials:

```json
{
  "registry": "registry.fly.io",
  "username": "x",
  "token": "<org-level-api-token>",
  "repository": "systemprompt-images",
  "tag": "tenant-{tenant_id}"
}
```

### Why This is Safe

1. **Fly.io deploy token scope**: Limited to `systemprompt-images` app only
2. **Empty app**: No machines, secrets, or volumes to compromise
3. **Exact validation**: Deploy rejects any image not matching `tenant-{id}` exactly
4. **Ownership checks**: Multi-layer auth before token issuance
5. **UUID tags**: Cryptographically hard to guess other tenants' tags

### Code References

| Component | File | Lines |
|-----------|------|-------|
| Image tag building | `services/fly/config.rs` | 101-111 |
| Registry token generation | `api/tenants/handlers.rs` | 331-363 |
| Deploy validation | `api/tenants/handlers.rs` | 205-221 |
| Validation logic | `models/tenant.rs` | 215-222 |
| Tenant ownership check | `api/tenants/mod.rs` | 25-30 |
| Tenant access control | `api/tenants/mod.rs` | 67-84 |
| Tag cleanup on deletion | `api/admin/sync.rs` | 139-143, 183-187 |

## Phase 1: Tenant Provisioning (`just tenant`)

### 1.1 Command Flow

```bash
# In systemprompt-template/
$ just tenant
# Executes: systemprompt cloud tenant
```

The CLI presents an interactive menu:
1. **Local Tenant**: Creates a PostgreSQL container locally
2. **Cloud Tenant**: Provisions a full Fly.io VM with payment

### 1.2 Cloud Tenant Creation Flow

#### Step 1: Authentication
```
CLI reads: .systemprompt/credentials.json
{
  "token": "eyJ...",  // JWT from login
  "user_id": "uuid",
  "email": "user@example.com"
}
```

#### Step 2: Plan Selection
```
CLI calls: GET /api/v1/plans
Returns available plans with pricing:
- Free tier
- Basic ($X/month)
- Pro ($X/month)
```

#### Step 3: Checkout Session
```
CLI calls: POST /api/v1/checkout
{
  "price_id": "pri_xxx",
  "region": "iad",
  "redirect_uri": "http://localhost:8000/callback"
}

Returns: { "checkout_url": "https://checkout.paddle.com/..." }
```

#### Step 4: Payment & Webhook
1. Browser opens Paddle checkout
2. User completes payment
3. Paddle sends webhook to Management API
4. Management API triggers provisioning

#### Step 5: Fly.io Infrastructure Provisioning (Phase 1)

The Management API's Fly Provisioner creates infrastructure only (NO machine):

```rust
// In: systemprompt-db/crates/management-api/src/services/fly/provisioner.rs

1. Create App
   fly.create_app(app_name)

2. Create Volume (persistent storage for /app/services)
   fly.create_volume(app_name, "services", size_gb, region)

3. Allocate IPs
   fly.allocate_shared_ipv4(app_name)
   fly.allocate_ipv6(app_name)

4. Generate Secrets
   - jwt_secret: 64-char random string
   - database_url: postgresql://user:pass@host/db
   - database_password: hashed and stored

5. Add TLS Certificate
   fly.add_certificate(app_name, hostname)

// NOTE: Machine is NOT created here - that happens in Phase 2 after CLI pushes image
```

#### Step 6: Event Streaming (Phase 1 Complete)
```
CLI subscribes: GET /api/v1/tenants/{id}/events (SSE)

Events received:
- provisioning_started
- app_created
- volume_created
- secrets_stored
- certificate_added
- infrastructure_ready (status: "awaiting_deploy")
```

#### Step 7: Build & Deploy (Phase 2)

After receiving `infrastructure_ready`, the CLI:

```bash
# 1. Build Docker image with tenant-scoped tag
docker build -f .systemprompt/Dockerfile \
  -t registry.fly.io/systemprompt-images:tenant-{tenant_id} .

# 2. Get registry token
curl -X GET /api/v1/tenants/{id}/registry-token
# Returns: { registry, username, token, repository: "systemprompt-images", tag: "tenant-{id}" }

# 3. Push image to shared registry
docker login registry.fly.io -u x -p $TOKEN
docker push registry.fly.io/systemprompt-images:tenant-{tenant_id}

# 4. Trigger deploy (creates machine with exact image validation)
curl -X POST /api/v1/tenants/{id}/deploy \
  -d '{"image": "registry.fly.io/systemprompt-images:tenant-{tenant_id}"}'
```

The deploy endpoint creates the machine:

```rust
// Validate image matches expected tenant-scoped tag
let expected_image = provisioner.config().build_full_image(&tenant_id);
req.validate_image(&expected_image)?;  // EXACT match required

fly.create_machine(app_name, MachineConfig {
    image: provided_image, // registry.fly.io/systemprompt-images:tenant-{id}
    env: { DATABASE_URL, JWT_SECRET, ... },
    mounts: [{ volume: "services", path: "/app/services" }],
    memory_mb: 256-2048,
})
```

#### Step 8: Final Event
```
SSE: tenant_ready
{ status: "running", url: "https://..." }
```

#### Step 9: Local Storage

After successful provisioning, CLI saves:

**`.systemprompt/tenants.json`**
```json
{
  "tenants": [
    {
      "id": "ten_abc123",
      "name": "my-project",
      "type": "cloud",
      "region": "iad",
      "hostname": "my-project.fly.dev",
      "created_at": "2024-01-15T..."
    }
  ]
}
```

**`.systemprompt/profiles/<tenant_id>/profile.yaml`**
```yaml
name: ten_abc123
database:
  type: postgres
server:
  host: 0.0.0.0
  port: 8080
  api_server_url: https://my-project.fly.dev
cloud:
  tenant_id: ten_abc123
  enabled: true
secrets:
  secrets_path: ./secrets.json
  source: file
```

**`.systemprompt/profiles/<tenant_id>/secrets.json`**
```json
{
  "jwt_secret": "random64chars...",
  "database_url": "postgresql://user:pass@db.fly.dev:5432/tenant_db",
  "anthropic": null,
  "openai": null,
  "gemini": null,
  "github": null
}
```

## How Local Code Gets Into Docker

This section explains how code from the local `systemprompt-template` repository is packaged into the Docker image that runs on Fly.io.

### Repository Structure

```
systemprompt-template/
├── core/                          # Git submodule → systemprompt-core
│   ├── Cargo.toml                 # Rust workspace
│   ├── crates/                    # All Rust crates
│   │   ├── entry/cli/             # CLI binary
│   │   ├── app/services/          # Services runtime
│   │   └── ...
│   ├── web/                       # React frontend
│   │   ├── src/
│   │   └── dist/                  # Built output
│   └── target/release/            # Built binaries
│       └── systemprompt           # Main executable
│
├── extensions/                    # Extension modules
│   ├── blog/                      # Blog extension
│   └── mcp/                       # MCP servers (git submodules)
│       ├── admin/                 # Admin MCP server
│       ├── infrastructure/        # Infrastructure MCP server
│       └── system-tools/          # System tools MCP server
│
├── services/                      # Runtime configuration
│   ├── agents/                    # Agent definitions (YAML)
│   ├── skills/                    # Skill definitions
│   ├── config/                    # App configuration
│   ├── content/                   # Static content (blog, legal)
│   ├── ai/                        # AI model configuration
│   └── web/                       # Web configuration
│
└── .systemprompt/                 # Deployment configuration
    ├── Dockerfile                 # Production image definition
    ├── entrypoint.sh              # Container startup script
    ├── profiles/                  # Environment profiles
    └── tenants.json               # Tenant registry
```

### Build Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        LOCAL BUILD PIPELINE                                  │
└─────────────────────────────────────────────────────────────────────────────┘

  SOURCE CODE                    BUILD STEPS                    DOCKER IMAGE
  ───────────                    ───────────                    ────────────

  core/                          cargo build --release
  ├── crates/                    ─────────────────────►         /app/bin/
  │   └── entry/cli/                                            └── systemprompt
  └── Cargo.toml

  core/web/                      npm run build
  ├── src/                       ─────────────────────►         /app/web/
  └── package.json                                              └── (React assets)

  services/                      (direct copy)
  ├── agents/                    ─────────────────────►         /app/services/
  ├── skills/                                                   ├── agents/
  ├── config/                                                   ├── skills/
  └── content/                                                  ├── config/
                                                                └── content/

  extensions/mcp/                npm run build (each)
  ├── admin/                     ─────────────────────►         /app/extensions/
  ├── infrastructure/                                           └── mcp/
  └── system-tools/                                                 ├── admin/
                                                                    ├── infrastructure/
                                                                    └── system-tools/

  .systemprompt/                 (direct copy)
  ├── entrypoint.sh              ─────────────────────►         /app/entrypoint.sh
  └── profiles/                                                 /app/services/profiles/
```

### Component Details

#### 1. Core Binary (`core/` → `/app/bin/systemprompt`)

The main Rust binary is built from the `systemprompt-core` submodule:

```bash
# Build command
cargo build --release --manifest-path=core/Cargo.toml

# Output location
core/target/release/systemprompt

# Docker destination
/app/bin/systemprompt
```

This binary contains:
- HTTP API server
- Database migrations
- Service runtime
- CLI commands

#### 2. Web Frontend (`core/web/` → `/app/web/`)

The React frontend is built separately:

```bash
# Build command
cd core/web && npm run build

# Output location
core/web/dist/

# Docker destination
/app/web/
```

Contains:
- React application bundle
- Static assets (CSS, images)
- index.html

#### 3. Services Configuration (`services/` → `/app/services/`)

Runtime configuration files are copied directly without transformation:

| Source | Destination | Contents |
|--------|-------------|----------|
| `services/agents/*.yaml` | `/app/services/agents/` | Agent definitions |
| `services/skills/*/` | `/app/services/skills/` | Skill configs and prompts |
| `services/config/*.yaml` | `/app/services/config/` | App configuration |
| `services/content/` | `/app/services/content/` | Blog posts, legal pages |
| `services/ai/config.yaml` | `/app/services/ai/` | AI model settings |
| `services/web/` | `/app/services/web/` | Web templates |

#### 4. MCP Extensions (`extensions/mcp/` → `/app/extensions/mcp/`)

MCP servers are Node.js/TypeScript projects that need to be built:

```bash
# For each MCP server
cd extensions/mcp/admin && npm install && npm run build
cd extensions/mcp/infrastructure && npm install && npm run build
cd extensions/mcp/system-tools && npm install && npm run build

# Output
extensions/mcp/*/dist/

# Docker destination
/app/extensions/mcp/*/
```

Each MCP server provides tools for AI agents:
- **admin**: User management, system configuration
- **infrastructure**: Fly.io, database operations
- **system-tools**: File operations, shell commands

#### 5. Profiles (`.systemprompt/profiles/` → `/app/services/profiles/`)

Environment-specific configuration:

```yaml
# Profile structure
.systemprompt/profiles/<tenant>/
├── profile.yaml    # Environment settings
└── secrets.json    # Sensitive credentials (not in image)
```

### Complete Dockerfile

```dockerfile
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates curl libpq5 libssl3 nodejs npm \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 app
WORKDIR /app
RUN mkdir -p /app/bin /app/web /app/services /app/extensions /app/data /app/logs

# Copy pre-built Rust binary
COPY target/release/systemprompt /app/bin/

# Copy pre-built React frontend
COPY core/web/dist /app/web

# Copy services configuration
COPY services /app/services

# Copy MCP extensions (pre-built)
COPY extensions/mcp /app/extensions/mcp

# Copy profiles
COPY .systemprompt/profiles /app/services/profiles

# Copy entrypoint
COPY .systemprompt/entrypoint.sh /app/entrypoint.sh

# Set permissions
RUN chmod +x /app/bin/* /app/entrypoint.sh && chown -R app:app /app

USER app
EXPOSE 8080

# Environment
ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=info \
    PATH="/app/bin:$PATH" \
    SYSTEMPROMPT_SERVICES_PATH=/app/services \
    SYSTEMPROMPT_EXTENSIONS_PATH=/app/extensions \
    WEB_DIR=/app/web

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/v1/health || exit 1

CMD ["/app/bin/systemprompt", "services", "serve", "--foreground"]
```

### What's NOT in the Docker Image

These are injected at runtime via environment variables:

| Item | Source | Injected By |
|------|--------|-------------|
| `DATABASE_URL` | Generated during provisioning | Fly provisioner |
| `JWT_SECRET` | Generated during provisioning | Fly provisioner |
| `ANTHROPIC_API_KEY` | User configuration | Fly secrets |
| `OPENAI_API_KEY` | User configuration | Fly secrets |
| Persistent data | Volume mount | Fly volume at `/app/services` |

---

## Phase 2: Code Deployment (`just deploy`)

### 2.1 Build Process

```bash
# In systemprompt-template/
$ just deploy

# Step 1: Build Rust binary
cargo build --release --manifest-path=core/Cargo.toml

# Step 2: Build React frontend
cd core/web && npm run build

# Step 3: Build Docker image with tenant-scoped tag
docker build -f .systemprompt/Dockerfile \
  -t registry.fly.io/systemprompt-images:tenant-{tenant_id} .
```

### 2.2 Docker Image Contents

**Dockerfile** (`.systemprompt/Dockerfile`):
```dockerfile
FROM debian:bookworm-slim

# Create non-root user
RUN useradd -m -u 1000 app

# Copy pre-built binary
COPY --chown=app:app core/target/release/systemprompt /app/bin/systemprompt

# Copy pre-built web assets
COPY --chown=app:app core/web/dist /app/web

# Copy services configuration
COPY --chown=app:app services /app/services

# Copy profiles (optional)
COPY --chown=app:app .systemprompt/profiles /app/services/profiles

# Copy entrypoint
COPY --chown=app:app .systemprompt/entrypoint.sh /app/entrypoint.sh

# Environment
ENV HOST=0.0.0.0
ENV PORT=8080
ENV RUST_LOG=info
ENV SYSTEMPROMPT_SERVICES_PATH=/app/services
ENV WEB_DIR=/app/web
ENV PATH="/app/bin:$PATH"

USER app
WORKDIR /app

ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["systemprompt", "services", "serve", "--foreground"]

HEALTHCHECK --interval=30s --timeout=5s \
  CMD curl -f http://localhost:8080/api/v1/health || exit 1
```

**Entrypoint Script** (`entrypoint.sh`):
```bash
#!/bin/bash
set -e

# Run database migrations
systemprompt services db migrate

# Start services
exec "$@"
```

### 2.3 Image Push & Deploy

```bash
# Get registry token (returns tenant-scoped tag info)
REGISTRY_INFO=$(curl -X GET /api/v1/tenants/{id}/registry-token)
# Returns: { registry, username, token, repository: "systemprompt-images", tag: "tenant-{id}" }

# Docker login to shared registry
docker login registry.fly.io -u x -p $TOKEN

# Push image to shared registry with tenant-scoped tag
docker push registry.fly.io/systemprompt-images:tenant-{tenant_id}

# Trigger deployment (image must match EXACTLY)
curl -X POST /api/v1/tenants/{id}/deploy \
  -d '{"image": "registry.fly.io/systemprompt-images:tenant-{tenant_id}"}'
```

The Management API then:
1. **Validates image** - Must match `registry.fly.io/systemprompt-images:tenant-{tenant_id}` exactly
2. Stops the running machine
3. Updates machine config with new image
4. Restarts machine
5. Returns deployment status

## Data Storage Summary

### Management API Database (systemprompt-db)

**`tenants` table**
```sql
id              UUID PRIMARY KEY
name            TEXT UNIQUE          -- "my-project"
database_name   TEXT UNIQUE          -- PostgreSQL database
user_name       TEXT UNIQUE          -- PostgreSQL user
password_hash   TEXT                 -- Hashed DB password
status          TEXT                 -- active, suspended, deleted
owner_customer_id TEXT FK            -- Paddle customer
plan_id         UUID FK              -- Subscription plan
region          TEXT                 -- "iad", "ord", etc.
memory_mb       INT                  -- 256, 512, 1024, 2048
fly_app_name    TEXT                 -- Fly app identifier
fly_machine_id  TEXT                 -- Running machine ID
fly_volume_id   TEXT                 -- Persistent volume ID
fly_hostname    TEXT                 -- "app.fly.dev"
fly_status      TEXT                 -- pending, running, failed
```

**`tenant_secrets` table**
```sql
tenant_id       UUID PRIMARY KEY
token           TEXT                 -- One-time retrieval token
database_url    TEXT                 -- Full connection string
jwt_secret      TEXT                 -- JWT signing key
created_at      TIMESTAMPTZ
```

**`subscriptions` table**
```sql
id              UUID PRIMARY KEY
customer_id     TEXT                 -- Paddle customer ID
paddle_subscription_id TEXT          -- Paddle subscription ID
status          TEXT                 -- active, cancelled, etc.
plan_id         UUID FK
current_period_start TIMESTAMPTZ
current_period_end   TIMESTAMPTZ
```

### Local Storage (systemprompt-template)

```
.systemprompt/
├── credentials.json        # API authentication token
├── tenants.json           # List of all tenants (local + cloud)
├── docker/
│   ├── local.yaml         # Docker Compose for local DB
│   └── <tenant>.yaml      # Per-tenant DB config
└── profiles/
    ├── local/
    │   ├── profile.yaml   # Profile configuration
    │   └── secrets.json   # Local secrets (DB URL, API keys)
    └── <cloud_tenant>/
        ├── profile.yaml   # Cloud profile config
        └── secrets.json   # Cloud secrets (fetched once)
```

## Environment Variables Injected at Runtime

The Fly provisioner injects these environment variables into the running machine:

### Core System
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
JWT_ACCESS_TOKEN_EXPIRATION=3600
```

### API Configuration
```
API_SERVER_URL=http://0.0.0.0:8080
API_EXTERNAL_URL=https://app.fly.dev
APP_URL=https://app.fly.dev
```

### Tenant Identity
```
TENANT_ID=ten_abc123
TENANT_NAME=my-project
```

### AI Provider Keys (optional)
```
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GEMINI_API_KEY=AIza...
```

## Secret Rotation

To rotate database credentials:

```bash
# CLI initiates rotation
PATCH /api/v1/tenants/{id}/rotate-credentials

# Management API:
# 1. Generates new password
# 2. Updates PostgreSQL user
# 3. Updates Fly machine secret (DATABASE_URL)
# 4. Machine restarts with new credentials
```

## Key Files by Repository

### systemprompt-template
| File | Purpose |
|------|---------|
| `justfile` | CLI command aliases |
| `.systemprompt/Dockerfile` | Production Docker image |
| `.systemprompt/entrypoint.sh` | Container startup script |
| `.systemprompt/tenants.json` | Local tenant registry |
| `.systemprompt/profiles/*/` | Per-tenant configuration |
| `services/` | Service definitions (copied to image) |

### systemprompt-core
| File | Purpose |
|------|---------|
| `crates/entry/cli/src/cloud/tenant.rs` | Tenant CLI commands |
| `crates/entry/cli/src/cloud/deploy.rs` | Deploy command |
| `crates/entry/cli/src/setup/secrets.rs` | Secrets management |
| `crates/infra/cloud/src/api_client/` | Management API client |
| `crates/infra/cloud/src/checkout/` | Paddle checkout flow |
| `crates/app/sync/src/crate_deploy.rs` | Docker build/push |

### systemprompt-db
| File | Purpose |
|------|---------|
| `crates/management-api/src/main.rs` | API server entry |
| `crates/management-api/src/routes.rs` | API endpoints |
| `crates/management-api/src/services/fly/provisioner.rs` | Fly.io provisioning |
| `crates/management-api/src/services/fly/environment.rs` | Env var registry |
| `crates/management-api/src/models/tenant.rs` | Tenant data model |
| `crates/management-api/migrations/*.sql` | Database schema |

## Troubleshooting

### Docker Missing Config/Code

The Docker image is self-contained. At runtime, it requires only:
1. **Environment variables** (injected by Fly provisioner)
2. **Volume mount** at `/app/services` (for persistent data)

If the container lacks configuration:
- Verify environment variables are set on the Fly machine
- Check the volume is mounted correctly
- Ensure the base image was built with all necessary files

### Secrets Not Available

Secrets follow this flow:
1. Generated by Management API during provisioning
2. Stored in `tenant_secrets` table (one-time retrieval)
3. Injected as Fly machine environment variables
4. CLI retrieves once and saves to local `secrets.json`

If secrets are missing:
- Check `tenant_secrets` table for the tenant
- Verify Fly machine has correct env vars: `fly ssh console -a {app} -- printenv`
- Re-fetch secrets: `systemprompt cloud tenant show --secrets`

---

## Detailed Sub-Flow Diagrams

### Paddle Checkout Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PADDLE CHECKOUT FLOW                                │
└─────────────────────────────────────────────────────────────────────────────┘

  CLI                    MANAGEMENT API              PADDLE                USER
  ───                    ──────────────              ──────                ────

  1. Create checkout
  ─────────────────►
  POST /api/v1/checkout
  {
    price_id: "pri_xxx",
    region: "iad",
    redirect_uri: "http://localhost:8000"
  }
                         2. Create Paddle session
                         ─────────────────────────►
                         Paddle API: POST /transactions

                         ◄─────────────────────────
                         { checkout_url, transaction_id }

  ◄─────────────────
  { checkout_url }

  3. Open browser
  ─────────────────────────────────────────────────────────────────────────►
                                                    checkout.paddle.com

                                                    4. User enters
                                                       payment details
                                                    ─────────────────►

                                                    5. Payment processed
                                                    ◄─────────────────

                         6. Webhook: transaction.completed
                         ◄─────────────────────────
                         POST /api/v1/webhooks/paddle
                         {
                           event_type: "transaction.completed",
                           data: { customer_id, subscription_id }
                         }

                         7. Create tenant record
                         8. Start Fly provisioning
                         9. Store subscription

  10. SSE: provisioning events
  ◄─────────────────
  GET /api/v1/tenants/{id}/events

                                                    11. Redirect to
                                                        localhost:8000
                                                    ◄─────────────────
```

### Fly.io Two-Phase Provisioning Sequence

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  PHASE 1: INFRASTRUCTURE (Backend)                           │
└─────────────────────────────────────────────────────────────────────────────┘

  MANAGEMENT API                              FLY.IO API
  ──────────────                              ──────────

  1. Create App
  ─────────────────────────────────────────►
  POST /apps
  { name: "sp-{tenant_id}", org: "systemprompt" }
                                              ◄──────
                                              { app_id, hostname }
  emit: app_created

  2. Create Volume
  ─────────────────────────────────────────►
  POST /apps/{app}/volumes
  { name: "services", size_gb: 1, region: "iad" }
                                              ◄──────
                                              { volume_id }
  emit: volume_created

  3. Allocate IPv4 (shared)
  ─────────────────────────────────────────►
  POST /apps/{app}/ips
  { type: "shared_v4" }
                                              ◄──────
                                              { ip: "x.x.x.x" }

  4. Allocate IPv6
  ─────────────────────────────────────────►
  POST /apps/{app}/ips
  { type: "v6" }
                                              ◄──────
                                              { ip: "xxxx::xxxx" }

  5. Generate secrets locally:
     - jwt_secret = random(64)
     - db_password = random(32)
     - database_url = postgresql://...

  6. Add TLS Certificate
  ─────────────────────────────────────────►
  POST /apps/{app}/certificates
  { hostname: "tenant.fly.dev" }
                                              ◄──────
                                              { certificate_id }

  emit: infrastructure_ready
  ─────────────────────────────────────────►
  { status: "awaiting_deploy", app_id: "sp-xxx" }

  ** NO MACHINE CREATED YET **

┌─────────────────────────────────────────────────────────────────────────────┐
│                  PHASE 2: DEPLOY (CLI-driven)                                │
└─────────────────────────────────────────────────────────────────────────────┘

  CLI                                         SHARED REGISTRY
  ───                                         ───────────────

  1. Build Docker image
  docker build -t registry.fly.io/systemprompt-images:tenant-{id}

  2. Push to shared registry
  ─────────────────────────────────────────►
  docker push registry.fly.io/systemprompt-images:tenant-{id}
                                              ◄──────
                                              { digest }

  CLI                       MANAGEMENT API                   FLY.IO API
  ───                       ──────────────                   ──────────

  3. Trigger deploy
  ─────────────────►
  POST /tenants/{id}/deploy
  { image: "registry.fly.io/systemprompt-images:tenant-{id}" }

                            4. Validate image (EXACT match)
                            expected = "registry.fly.io/systemprompt-images:tenant-{id}"
                            if req.image != expected { reject }

                            5. Create Machine
                            ─────────────────────────────────────────►
                            POST /apps/{app}/machines
                            {
                              config: {
                                image: "registry.fly.io/systemprompt-images:tenant-{id}",
                                env: { DATABASE_URL, JWT_SECRET, ... },
                                mounts: [{ volume, path: "/app/services" }]
                              }
                            }
                                                                    ◄──────
                                                                    { machine_id }

                            6. Wait for running
                            ─────────────────────────────────────────►
                            GET /apps/{app}/machines/{id}
                                                                    ◄──────
                                                                    { state: "started" }

  ◄─────────────────
  { status: "deployed", url: "https://..." }

  emit: tenant_ready
```

### Deploy Sequence (Code Update)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          DEPLOY SEQUENCE                                     │
└─────────────────────────────────────────────────────────────────────────────┘

  LOCAL MACHINE                MANAGEMENT API           SHARED REGISTRY    FLY.IO
  ─────────────                ──────────────           ───────────────    ──────

  1. Build Rust binary
  cargo build --release
  └── target/release/systemprompt

  2. Build React frontend
  cd core/web && npm run build
  └── core/web/dist/

  3. Build MCP extensions
  cd extensions/mcp/* && npm run build
  └── extensions/mcp/*/dist/

  4. Build Docker image (tenant-scoped tag)
  docker build -f .systemprompt/Dockerfile \
    -t registry.fly.io/systemprompt-images:tenant-{id} .

  5. Get registry token
  ─────────────────────────►
  GET /api/v1/tenants/{id}/registry-token
  ◄─────────────────────────
  { registry, token, repository: "systemprompt-images", tag: "tenant-{id}" }

  6. Push image to shared registry
  ───────────────────────────────────────►
  docker push registry.fly.io/systemprompt-images:tenant-{id}
                                           ◄──────
                                           { digest }

  7. Trigger deployment
  ─────────────────────────►
  POST /api/v1/tenants/{id}/deploy
  { image: "registry.fly.io/systemprompt-images:tenant-{id}" }

                             8. Validate image (EXACT match)
                             expected = build_full_image(tenant_id)
                             if req.image != expected { return 400 }

                             9. Update machine
                             ──────────────────────────────────────────────►
                             PATCH /apps/{app}/machines/{id}
                             { config: { image: "registry.fly.io/systemprompt-images:tenant-{id}" } }

                                                                           10. Pull image
                                                                           ◄──────
                                                                           (from shared registry)

                                                                           11. Restart machine
                                                                           ◄──────
                             ◄──────────────────────────────────────────────
                             { machine_id, state: "started" }

  ◄─────────────────────────
  { status: "deployed", url: "https://..." }
```

### Secrets Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            SECRETS FLOW                                      │
└─────────────────────────────────────────────────────────────────────────────┘

  PROVISIONING (one-time)
  ───────────────────────

  Management API                                     Database
  ──────────────                                     ────────

  1. Generate secrets:
     jwt_secret = secure_random(64)
     db_password = secure_random(32)
     database_url = format(...)

  2. Store in tenant_secrets table
  ─────────────────────────────────────────────────►
  INSERT INTO tenant_secrets
  (tenant_id, token, database_url, jwt_secret)
  VALUES (...)
                                                     ◄──────
                                                     (stored encrypted)

  3. One-time retrieval token issued to CLI

  CLI RETRIEVAL (one-time)
  ────────────────────────

  CLI                          Management API                Database
  ───                          ──────────────                ────────

  1. Fetch secrets
  ─────────────────────────►
  GET /api/v1/tenants/{id}/secrets
  Authorization: Bearer {token}

                               2. Validate one-time token
                               ─────────────────────────────────────────►
                               SELECT * FROM tenant_secrets
                               WHERE tenant_id = ? AND token = ?
                                                                         ◄──────
                                                                         { secrets }

                               3. Delete one-time token
                               ─────────────────────────────────────────►
                               DELETE FROM tenant_secrets WHERE tenant_id = ?

  ◄─────────────────────────
  { database_url, jwt_secret }

  4. Save locally
  └── .systemprompt/profiles/{id}/secrets.json


  SECRET ROTATION
  ───────────────

  CLI                    Management API              Fly.io             Database
  ───                    ──────────────              ──────             ────────

  1. Request rotation
  ─────────────────►
  PATCH /api/v1/tenants/{id}/rotate-credentials

                         2. Generate new password
                         new_password = secure_random(32)

                         3. Update PostgreSQL
                         ─────────────────────────────────────────────────►
                         ALTER USER {user} PASSWORD '{new_password}'

                         4. Update Fly secrets
                         ──────────────────────►
                         PATCH /apps/{app}/machines/{id}
                         { config: { env: { DATABASE_URL: "..." } } }
                                                    ◄──────
                                                    (machine restarts)

  ◄─────────────────
  { status: "rotated" }

  5. Fetch new secrets (same as retrieval flow)
```

---

## Operational Runbook

> **Note**: Tenants do NOT have direct access to Fly.io. All operations go through the SystemPrompt CLI or Management API.

### Create New Cloud Tenant

```bash
# 1. Ensure you're logged in
cd /var/www/html/systemprompt-template
just login

# 2. Create tenant (interactive)
just tenant
# Select: Create → Cloud → Choose plan → Choose region

# 3. Complete payment in browser
# Wait for provisioning events...

# 4. Verify tenant created
just tenant
# Select: List → Verify new tenant appears

# 5. Check tenant status
just tenant
# Select: Show → Select tenant → View details
```

### Deploy Code Updates

```bash
# 1. Ensure all changes are committed
cd /var/www/html/systemprompt-template
git status

# 2. Build everything locally first
just build --release

# 3. Deploy to specific tenant
just deploy --tenant <tenant_id>

# Or deploy interactively
just deploy
# Select tenant from list

# 4. Monitor deployment
# Watch for success message with URL
```

### Check Tenant Status

```bash
# Via CLI
just tenant
# Select: Show → Select tenant

# Via API
curl -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>

# Get detailed status including Fly machine state
curl -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/status
```

### View Logs

> **Note**: Logs endpoint is planned but not yet implemented. Current workaround:

```bash
# Check tenant status for basic info
curl -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/status

# Check metrics for resource usage
curl -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/metrics
```

### Rotate Credentials

```bash
# Via CLI
just tenant
# Select: Edit → Select tenant → Rotate credentials

# Via API (POST, not PATCH)
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/rotate-credentials

# After rotation, fetch new secrets
just tenant
# Select: Show → Select tenant → Download secrets
```

### Scale Resources

```bash
# View current allocation
just tenant
# Select: Show → Select tenant → View resources

# Scale via API
curl -X PATCH \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"memory_mb": 512}' \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>

# Available memory options: 256, 512, 1024, 2048
```

### Suspend/Delete Tenant

```bash
# Suspend (keeps data, stops billing)
just tenant
# Select: Edit → Select tenant → Suspend

# Via API - Suspend:
curl -X PATCH \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"status": "suspended"}' \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>

# Delete (permanent, data loss)
just tenant
# Select: Delete → Select tenant → Confirm

# Via API - Delete:
curl -X DELETE \
  -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>
```

### Recover from Failed Provisioning

```bash
# 1. Check provisioning status
curl -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/status

# 2. View provisioning events (SSE stream)
curl -H "Authorization: Bearer $TOKEN" \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/events

# 3. If completely failed, delete and start fresh
just tenant
# Select: Delete → Select failed tenant → Confirm
# Then create new tenant
```

> **Note**: Retry provisioning endpoint (`POST /retry-provision`) is planned but not yet implemented. Currently, failed tenants must be deleted and recreated.

### Set Secrets (API Keys)

```bash
# Set AI provider keys via CLI
just tenant
# Select: Edit → Select tenant → Set secrets

# Via API
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"key": "ANTHROPIC_API_KEY", "value": "sk-ant-..."}' \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/secrets

# Available secret keys:
# - ANTHROPIC_API_KEY
# - OPENAI_API_KEY
# - GEMINI_API_KEY
# - GITHUB_TOKEN
```

### Restart Tenant

> **Note**: Restart endpoint is planned but not yet implemented. Current workaround:

```bash
# Trigger a redeploy with same image (effectively restarts)
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"image": "current"}' \
  https://api.systemprompt.io/api/v1/tenants/<tenant_id>/deploy
```

### Common Issues and Solutions

| Issue | Cause | Solution |
|-------|-------|----------|
| Machine won't start | Memory too low | Scale to 512MB via API |
| Health check failing | App crash on startup | Check logs via `just logs` |
| Database connection error | Wrong DATABASE_URL | Rotate credentials via API |
| Certificate not working | DNS not propagated | Wait 5-10 minutes |
| Deploy stuck | Registry auth expired | Re-run `just login` |
| API keys not working | Secrets not set | Set via tenant secrets API |

### API Endpoints Reference

> **Note**: Tenants are created automatically via Paddle webhook when a subscription is purchased, not via direct API call.

#### Implemented Endpoints

| Operation | Method | Endpoint |
|-----------|--------|----------|
| List tenants | GET | `/api/v1/tenants` |
| Get tenant | GET | `/api/v1/tenants/{id}` |
| Update tenant | PATCH | `/api/v1/tenants/{id}` |
| Delete tenant | DELETE | `/api/v1/tenants/{id}` |
| Suspend tenant | POST | `/api/v1/tenants/{id}/suspend` |
| Activate tenant | POST | `/api/v1/tenants/{id}/activate` |
| Get status | GET | `/api/v1/tenants/{id}/status` |
| Stream events | GET | `/api/v1/tenants/{id}/events` (SSE) |
| Get environment | GET | `/api/v1/tenants/{id}/environment` |
| Deploy | POST | `/api/v1/tenants/{id}/deploy` |
| Get registry token | GET | `/api/v1/tenants/{id}/registry-token` |
| Rotate credentials | POST | `/api/v1/tenants/{id}/rotate-credentials` |
| Get secrets (one-time) | GET | `/api/v1/tenants/{id}/credentials/{token}` |
| List secrets | GET | `/api/v1/tenants/{id}/secrets` |
| Set secret | POST | `/api/v1/tenants/{id}/secrets` |
| Get secret | GET | `/api/v1/tenants/{id}/secrets/{key}` |
| Update secret | PATCH | `/api/v1/tenants/{id}/secrets/{key}` |
| Delete secret | DELETE | `/api/v1/tenants/{id}/secrets/{key}` |
| Get metrics | GET | `/api/v1/tenants/{id}/metrics` |
| Get metrics history | GET | `/api/v1/tenants/{id}/metrics/history` |

#### Checkout & Subscription Endpoints

| Operation | Method | Endpoint |
|-----------|--------|----------|
| Create checkout | POST | `/api/v1/checkout` |
| List plans | GET | `/api/v1/checkout/plans` |
| Get subscription | GET | `/api/v1/tenants/{id}/subscription` |
| Create subscription | POST | `/api/v1/tenants/{id}/subscription` |
| Cancel subscription | POST | `/api/v1/tenants/{id}/subscription/cancel` |

#### Planned Endpoints (Not Yet Implemented)

| Operation | Method | Endpoint | Status |
|-----------|--------|----------|--------|
| Get logs | GET | `/api/v1/tenants/{id}/logs` | Planned |
| Restart | POST | `/api/v1/tenants/{id}/restart` | Planned |
| Retry provision | POST | `/api/v1/tenants/{id}/retry-provision` | Planned |

See `instructions/plan/systemprompt-template-cloud-fixes.md` for implementation details.
