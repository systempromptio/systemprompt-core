# Configuration System

This document describes how profiles, secrets, credentials, and tenants combine to create a validated configuration in both development and production environments.

---

## Core Design Principles

1. **Profiles are the single source of truth** - All configuration comes from YAML files, not environment variables
2. **Environment-aware loading** - Secrets and credentials load from JSON files (dev) or env vars (Fly.io prod)
3. **Separation of concerns**:
   - **Profiles** - Environment configuration (paths, database, server settings, rate limits)
   - **Secrets** - JWT secret + AI provider API keys (NEVER in profiles)
   - **Credentials** - Cloud authentication only (cloud auth JWT token)
   - **Tenants** - Cached tenant list from API
4. **Validation at startup** - Strict validation with typed models at app entry
5. **Secrets are required** - JWT secret must be present for app to start

---

## File Structure

```
.systemprompt/
├── profiles/
│   ├── dev.profile.yml           # Development profile
│   ├── staging.profile.yml       # Staging profile
│   └── prod.profile.yml          # Production profile
├── secrets/
│   ├── dev.secrets.json          # Dev secrets (JWT secret + API keys)
│   ├── staging.secrets.json      # Staging secrets
│   └── prod.secrets.json         # Production secrets
├── credentials/
│   └── *.credentials.json        # Cloud auth tokens
└── tenants.json                  # Cached tenant list from API
```

**Key principle**: Each environment has its own profile AND secrets file. Profiles reference their secrets file via `secrets.secrets_path`.

---

## Bootstrap Sequence

```
ProfileBootstrap::init()       // 1. Load profile YAML from SYSTEMPROMPT_PROFILE
        ↓
SecretsBootstrap::init()       // 2. Load secrets.json (optional, non-fatal)
        ↓
CredentialsBootstrap::init()   // 3. Load credentials.json (auth only)
        ↓
TenantsBootstrap::init()       // 4. Load tenants.json (cached tenant list)
        ↓
Config::init()                 // 5. Build Config, resolve tenant from cache
        ↓
AppContext::new()              // 6. Initialize database, extensions, services
```

**Key files:**
- `crates/shared/models/src/profile_bootstrap.rs` - Profile initialization
- `crates/shared/models/src/secrets.rs` - Secrets initialization (SecretsBootstrap)
- `crates/infra/cloud/src/credentials.rs` - Cloud credentials (CredentialsBootstrap)
- `crates/infra/cloud/src/tenants.rs` - Tenant cache
- `crates/shared/models/src/config/mod.rs` - Config struct and validation
- `crates/entry/cli/src/bootstrap.rs` - CLI bootstrap sequence

---

## Configuration Components

### 1. Profile (YAML)

The master configuration containing all environment-specific settings.

```yaml
name: local                    # Profile identifier
display_name: Local Development

site:
  name: my-app
  github_link: null

database:
  type: postgres
  url: postgres://user:pass@localhost:5432/mydb

server:
  host: 127.0.0.1
  port: 8080
  api_server_url: http://localhost:8080
  api_internal_url: http://localhost:8080
  api_external_url: http://localhost:8080
  use_https: false
  cors_allowed_origins:
    - http://localhost:8080

paths:
  system: /var/www/html/myapp
  core: /var/www/html/myapp/core
  services: /var/www/html/myapp/services
  # ... additional paths

security:
  jwt_issuer: myapp-local
  jwt_access_token_expiration: 86400
  jwt_refresh_token_expiration: 2592000
  # NOTE: jwt_secret is NOT here - it's in secrets.json

secrets:                       # References secrets.json
  secrets_path: ../secrets/dev.secrets.json
  validation: warn             # strict | warn | skip

cloud:                         # Cloud configuration
  credentials_path: ./credentials.json
  tenant_id: tenant_123        # Reference to tenant (looked up from tenants.json)
  enabled: true
  validation: warn             # strict | warn | skip

rate_limits:
  disabled: true               # Typically disabled in dev
  # ... per-endpoint limits

runtime:
  environment: development
  log_level: verbose
  output_format: text
```

**Location:** Profile path is specified via `SYSTEMPROMPT_PROFILE` environment variable.

**Path Resolution:** Relative paths in profiles are resolved relative to the profile file's directory.

**Environment Variable Substitution:** Profiles support `${VAR_NAME}` syntax for env var substitution during YAML parsing.

---

### 2. Credentials (JSON) - Authentication Only

Cloud authentication credentials. **Contains auth info only, no tenant data.**

```json
{
  "api_token": "eyJhbG...",
  "api_url": "https://api.systemprompt.io",
  "authenticated_at": "2024-01-15T10:30:00Z",
  "user_email": "user@example.com"
}
```

**Location:** Specified in profile's `cloud.credentials_path` field.

**Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `api_token` | string | Yes | JWT token for API authentication |
| `api_url` | string | Yes | Cloud API endpoint URL |
| `authenticated_at` | datetime | Yes | When authentication occurred |
| `user_email` | string | No | User's email address |

**Validation:**
- `api_token` must be non-empty
- `api_url` must be a valid URL
- `user_email` must be valid email format (if present)
- JWT token expiration is checked on load

---

### 3. Tenants (JSON) - Cached Tenant List

Cached list of user's tenants from the API. Refreshed on login and config operations.

```json
{
  "tenants": [
    {
      "id": "tenant_123",
      "name": "My Tenant",
      "app_id": "app_456",
      "hostname": "myapp.systemprompt.io",
      "region": "iad"
    },
    {
      "id": "tenant_789",
      "name": "Other Tenant",
      "app_id": "app_012",
      "hostname": "other.systemprompt.io",
      "region": "lhr"
    }
  ],
  "fetched_at": "2024-01-15T10:30:00Z"
}
```

**Location:** `.systemprompt/tenants.json`

**Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tenants` | array | Yes | List of cached tenants |
| `fetched_at` | datetime | Yes | When tenants were fetched from API |

**Tenant Fields:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique tenant identifier |
| `name` | string | Yes | Display name |
| `app_id` | string | No | Fly.io app identifier |
| `hostname` | string | No | Deployment hostname |
| `region` | string | No | Deployment region |

---

### 4. Secrets (JSON) - JWT Secret + API Keys

Container for JWT secret and AI provider API keys. **JWT secret is REQUIRED**.

```json
{
  "jwt_secret": "64-character-random-string-generated-by-setup-wizard",
  "gemini": "AIzaSy...",
  "anthropic": "sk-ant-...",
  "openai": "sk-...",
  "github": "ghp_...",
  "custom_key": "value"
}
```

**Location:** Specified in profile's `secrets.secrets_path` field (e.g., `../secrets/dev.secrets.json`).

**File Permissions:** Set to `0o600` (read/write owner only) on Unix systems.

**Required Fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `jwt_secret` | **YES** | Token signing key (min 32 chars, recommended 64). App fails to start without this. |
| `gemini` | No* | Google AI (Gemini) API key |
| `anthropic` | No* | Anthropic (Claude) API key |
| `openai` | No* | OpenAI API key |
| `github` | No | GitHub token |

*At least one AI provider key is required for AI functionality.

**Validation Modes** (for file loading, NOT jwt_secret):
| Mode | Behavior |
|------|----------|
| `strict` | Fail startup if secrets file missing/invalid |
| `warn` | Log warning, continue with empty secrets (default) |
| `skip` | Skip validation entirely |

**Important:** JWT secret validation is ALWAYS enforced regardless of validation mode. If jwt_secret is missing, the application will fail to start.

---

## Command Behaviors

### Login (`systemprompt cloud login`)

1. Run OAuth flow → get JWT token
2. Create/update `credentials.json` (auth only)
3. Fetch user's tenants from API
4. Save to `tenants.json`
5. Display available tenants

**Does NOT:**
- Select a tenant (that happens in `cloud config`)
- Modify profiles

### Logout (`systemprompt cloud logout`)

1. Delete `credentials.json`

**Does NOT:**
- Delete `tenants.json` (stale but harmless cache)
- Delete profiles (still valid, just need re-auth)
- Delete secrets

### Config (`systemprompt cloud config`)

1. Check `credentials.json` exists (require login first)
2. Refresh `tenants.json` from API
3. User selects tenant
4. Generate/update profile with `tenant_id` reference
5. Generate secrets file
6. Setup database

---

## Profile-Tenant Relationship

Profiles reference tenants by ID. At runtime, tenant details are resolved from the cached `tenants.json`.

```yaml
# In profile
cloud:
  credentials_path: ./credentials.json
  tenant_id: tenant_123    # ← Reference, not embedded data
  enabled: true
  validation: warn
```

**Resolution at runtime:**
1. Load profile → get `cloud.tenant_id`
2. Load `tenants.json` → find tenant by ID
3. Merge into runtime context

**If tenant not found in cache:**
- In `strict` mode: Fail startup
- In `warn` mode: Log warning, cloud features disabled
- In `skip` mode: Continue silently

---

## Local Development Setup

```
myapp/
├── .systemprompt/
│   ├── credentials.json       # Cloud auth (from login)
│   ├── tenants.json           # Cached tenants (from login)
│   ├── secrets.json           # API keys
│   └── profiles/
│       ├── local.secrets.profile.yml
│       └── production.secrets.profile.yml
└── services/                  # Boilerplate
```

**Full Setup:**
```bash
# Step 1: Login to systemprompt.io Cloud
systemprompt cloud login

# Step 2: Run the config wizard
systemprompt cloud config
```

The `config` command will:
- Refresh tenant cache from API
- Prompt for tenant selection
- Prompt for AI provider API keys
- Configure PostgreSQL
- Generate profile with `tenant_id` reference
- Save secrets to `secrets.json`
- Run database migrations

**Environment variable:**
```bash
export SYSTEMPROMPT_PROFILE=.systemprompt/profiles/local.secrets.profile.yml
```

---

## Production Setup (Fly.io)

In production, secrets and credentials are loaded from environment variables.

### Set Fly Secrets

```bash
# API keys
fly secrets set \
  GEMINI_API_KEY="AIzaSy..." \
  ANTHROPIC_API_KEY="sk-ant-..." \
  OPENAI_API_KEY="sk-..."

# Cloud auth
fly secrets set \
  SYSTEMPROMPT_API_TOKEN="eyJhbG..." \
  SYSTEMPROMPT_API_URL="https://api.systemprompt.io" \
  SYSTEMPROMPT_USER_EMAIL="user@example.com"
```

### Environment Variables

**Credentials (Auth):**
| Environment Variable | Required | Default | Description |
|---------------------|----------|---------|-------------|
| `SYSTEMPROMPT_API_TOKEN` | Yes | - | JWT authentication token |
| `SYSTEMPROMPT_API_URL` | No | `https://api.systemprompt.io` | API endpoint |
| `SYSTEMPROMPT_USER_EMAIL` | No | - | User email |

**Secrets (API Keys):**
| Environment Variable | Maps To |
|---------------------|---------|
| `GEMINI_API_KEY` | `secrets.gemini` |
| `ANTHROPIC_API_KEY` | `secrets.anthropic` |
| `OPENAI_API_KEY` | `secrets.openai` |
| `GITHUB_TOKEN` | `secrets.github` |

**Detection:** Fly.io is detected via `FLY_APP_NAME` env var. When detected, credentials and secrets load from env vars instead of JSON files.

---

## Validation

### Typed Model Validation

All config structs implement strict validation using the `validator` crate:

```rust
#[derive(Validate)]
pub struct CloudCredentials {
    #[validate(length(min = 1))]
    pub api_token: String,

    #[validate(url)]
    pub api_url: String,

    pub authenticated_at: DateTime<Utc>,

    #[validate(email)]
    pub user_email: Option<String>,
}
```

### Bootstrap Validation Flow

```
1. Load credentials.json → parse into CloudCredentials → validate()
2. Load tenants.json → parse into TenantCache → validate()
3. Load profile → validate CloudConfig section
4. If profile.cloud.tenant_id set:
   - Lookup in TenantCache
   - Fail if tenant_id not found (in strict mode)
5. JWT token expiration check
6. Return validated, typed context
```

### Validation Modes

| Mode | Credentials | Tenants | Profile |
|------|------------|---------|---------|
| `strict` | Required, validated | Required if tenant_id set | Required |
| `warn` | Optional, log warning | Optional, log warning | Required |
| `skip` | Skip entirely | Skip entirely | Required |

---

## Accessing Configuration at Runtime

### Profile

```rust
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

let profile = ProfileBootstrap::get()?;
println!("Running profile: {}", profile.name);
println!("Tenant ID: {:?}", profile.cloud.tenant_id);
```

### Credentials

```rust
use systemprompt_cloud::CredentialsBootstrap;

if let Some(creds) = CredentialsBootstrap::get()? {
    println!("Logged in as: {:?}", creds.user_email);
    println!("API URL: {}", creds.api_url);
}
```

### Tenant (Resolved)

```rust
use systemprompt_cloud::{TenantsBootstrap, CredentialsBootstrap};

// Get cached tenants
if let Some(cache) = TenantsBootstrap::get()? {
    for tenant in &cache.tenants {
        println!("Tenant: {} ({})", tenant.name, tenant.id);
    }
}

// Lookup specific tenant
let tenant_id = profile.cloud.tenant_id.as_deref();
if let Some(id) = tenant_id {
    if let Some(tenant) = cache.find_tenant(id) {
        println!("Using tenant: {}", tenant.name);
    }
}
```

### Secrets

```rust
use systemprompt::SecretsBootstrap;

let secrets = SecretsBootstrap::get()?;
if let Some(key) = secrets.get("anthropic") {
    // Use API key
}
```

---

## Security Considerations

1. **Never commit secrets** - All JSON files in `.systemprompt/` should be in `.gitignore`
2. **File permissions** - JSON files created with `0o600` permissions (owner read/write only)
3. **JWT secret length** - Minimum 32 characters enforced at validation
4. **Fly.io secrets** - Use `fly secrets set` for secure env var management
5. **Validation modes** - Use `strict` in production, `warn` in development
6. **Token expiration** - JWT tokens are validated for expiration on load

---

## Troubleshooting

### Not Logged In

```
Not logged in to systemprompt.io Cloud.
Run 'systemprompt cloud login' first.
```

**Fix:** Run `systemprompt cloud login`

### Tenant Not Found

```
Tenant 'tenant_123' not found in cache.
Run 'systemprompt cloud config' to refresh tenants.
```

**Fix:** Run `systemprompt cloud config` to refresh tenant cache and select a tenant.

### Token Expired

```
Cloud token has expired. Run 'systemprompt cloud login' to refresh.
```

**Fix:** Run `systemprompt cloud login`

### Profile Not Found

```
Profile initialization failed. Set SYSTEMPROMPT_PROFILE environment variable.
```

**Fix:**
```bash
export SYSTEMPROMPT_PROFILE=.systemprompt/profiles/local.secrets.profile.yml
```

---

## Summary

| Component | Format | Dev Location | Prod Location | Contains |
|-----------|--------|--------------|---------------|----------|
| Profile | YAML | `.systemprompt/profiles/*.yml` | Container filesystem | All config + tenant_id ref |
| Credentials | JSON | `.systemprompt/credentials.json` | Fly env vars | Auth only |
| Tenants | JSON | `.systemprompt/tenants.json` | N/A (API fetch) | Cached tenant list |
| Secrets | JSON | `.systemprompt/secrets.json` | Fly env vars | API keys |

**Key Principles:**
- Credentials = authentication only (no tenant data)
- Profiles reference tenants by ID
- Tenant details resolved from cache at runtime
- Strict validation with typed models at app entry
