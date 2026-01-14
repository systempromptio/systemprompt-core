# Cloud CLI Commands

This document provides complete documentation for AI agents to use the cloud CLI commands. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `cloud auth login` | Authenticate with SystemPrompt Cloud | `Card` | No |
| `cloud auth logout` | Log out from cloud | `Text` | No |
| `cloud auth whoami` | Show current authentication status | `Card` | No |
| `cloud init` | Initialize project structure | `Text` | No |
| `cloud tenant` | Manage tenants (list/show) | `Table`/`Card` | No |
| `cloud tenant create` | Create a new tenant | `Text` | No |
| `cloud tenant select` | Select active tenant | `Text` | No |
| `cloud tenant rotate` | Rotate tenant API keys | `Text` | No |
| `cloud profile` | Manage profiles (list/show) | `Table`/`Card` | No |
| `cloud profile create` | Create a new profile | `Text` | No |
| `cloud profile edit` | Edit profile configuration | `Text` | No |
| `cloud profile delete` | Delete a profile | `Text` | No |
| `cloud deploy` | Deploy to SystemPrompt Cloud | `Text` | No |
| `cloud status` | Check cloud deployment status | `Card` | No |
| `cloud restart` | Restart tenant machine | `Text` | Yes |
| `cloud sync` | Sync between local and cloud | `Text` | Yes |
| `cloud secrets` | Manage cloud secrets | `Text`/`Table` | No |
| `cloud dockerfile` | Generate Dockerfile | `Text` | No |

---

## Authentication Commands

### cloud auth login

Authenticate with SystemPrompt Cloud.

```bash
sp cloud auth login
sp cloud auth login --email user@example.com --password "password"
sp cloud auth login --token "api_token"
```

**Optional Flags (non-interactive):**
| Flag | Description |
|------|-------------|
| `--email` | Email address |
| `--password` | Password |
| `--token` | API token (alternative to email/password) |

**Output Structure:**
```json
{
  "authenticated": true,
  "user": {
    "email": "user@example.com",
    "name": "User Name"
  },
  "tenant": "tenant_abc123",
  "message": "Successfully authenticated"
}
```

**Artifact Type:** `Card`

---

### cloud auth logout

Log out from SystemPrompt Cloud.

```bash
sp cloud auth logout
```

**Output Structure:**
```json
{
  "message": "Successfully logged out"
}
```

**Artifact Type:** `Text`

---

### cloud auth whoami

Show current authentication status.

```bash
sp cloud auth whoami
sp --json cloud auth whoami
```

**Output Structure:**
```json
{
  "authenticated": true,
  "email": "user@example.com",
  "name": "User Name",
  "tenant": "tenant_abc123",
  "expires_at": "2024-01-16T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

## Project Initialization

### cloud init

Initialize project structure for SystemPrompt.

```bash
sp cloud init
sp cloud init --force
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--force` | `false` | Overwrite existing configuration |

**Created Structure:**
```
.systemprompt/
├── profiles/
│   └── local/
│       ├── profile.yaml
│       └── secrets.yaml
├── services/
│   ├── agents/
│   └── mcp/
└── config.yaml
```

**Output Structure:**
```json
{
  "initialized": true,
  "path": "/var/www/html/project/.systemprompt",
  "message": "Project initialized successfully"
}
```

**Artifact Type:** `Text`

---

## Tenant Management Commands

### cloud tenant (list)

List all tenants.

```bash
sp cloud tenant
sp --json cloud tenant
```

**Output Structure:**
```json
{
  "tenants": [
    {
      "id": "tenant_abc123",
      "name": "My Project",
      "status": "active",
      "region": "us-east-1",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "active_tenant": "tenant_abc123"
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `name`, `status`, `region`, `created_at`

---

### cloud tenant create

Create a new tenant.

```bash
sp cloud tenant create --name "My Project"
sp cloud tenant create --name "My Project" --region us-west-2
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Tenant name |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--region` | `us-east-1` | Cloud region |

**Output Structure:**
```json
{
  "id": "tenant_abc123",
  "name": "My Project",
  "api_key": "sp_live_xxx...",
  "message": "Tenant created successfully"
}
```

**Artifact Type:** `Text`

---

### cloud tenant select

Select active tenant.

```bash
sp cloud tenant select <tenant-id>
sp cloud tenant select tenant_abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Tenant ID to select |

**Output Structure:**
```json
{
  "tenant_id": "tenant_abc123",
  "message": "Tenant 'tenant_abc123' selected"
}
```

**Artifact Type:** `Text`

---

### cloud tenant rotate

Rotate tenant API keys.

```bash
sp cloud tenant rotate --yes
sp cloud tenant rotate --tenant tenant_abc123 --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm key rotation |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--tenant` | Active tenant | Specific tenant ID |

**Output Structure:**
```json
{
  "tenant_id": "tenant_abc123",
  "new_api_key": "sp_live_new_xxx...",
  "old_key_revoked": true,
  "message": "API key rotated successfully"
}
```

**Artifact Type:** `Text`

---

## Profile Management Commands

### cloud profile (list)

List all profiles.

```bash
sp cloud profile
sp --json cloud profile
```

**Output Structure:**
```json
{
  "profiles": [
    {
      "name": "local",
      "path": "/var/www/html/tyingshoelaces/.systemprompt/profiles/local",
      "environment": "development",
      "active": true
    },
    {
      "name": "staging",
      "path": "/var/www/html/tyingshoelaces/.systemprompt/profiles/staging",
      "environment": "staging",
      "active": false
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `environment`, `active`

---

### cloud profile show

Show profile details.

```bash
sp cloud profile show <profile-name>
sp --json cloud profile show local
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Profile name |

**Output Structure:**
```json
{
  "name": "local",
  "path": "/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml",
  "environment": "development",
  "database": {
    "host": "localhost",
    "port": 5432,
    "name": "systemprompt_dev"
  },
  "secrets_configured": {
    "anthropic": true,
    "openai": false
  }
}
```

**Artifact Type:** `Card`

---

### cloud profile create

Create a new profile.

```bash
sp cloud profile create --name staging --environment staging
sp cloud profile create \
  --name production \
  --environment production \
  --db-host prod-db.example.com \
  --db-port 5432 \
  --db-name systemprompt_prod
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Profile name |
| `--environment` | Yes | Environment type |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--db-host` | `localhost` | Database host |
| `--db-port` | `5432` | Database port |
| `--db-name` | `systemprompt_<env>` | Database name |

**Output Structure:**
```json
{
  "name": "staging",
  "path": "/var/www/html/tyingshoelaces/.systemprompt/profiles/staging",
  "message": "Profile 'staging' created successfully"
}
```

**Artifact Type:** `Text`

---

### cloud profile edit

Edit profile configuration.

```bash
sp cloud profile edit <profile-name> --db-host new-host
sp cloud profile edit local --set database.port=5433
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Profile name |
| At least one change | Yes | Must specify at least one modification |

**Modification Flags:**
| Flag | Description |
|------|-------------|
| `--db-host` | Update database host |
| `--db-port` | Update database port |
| `--db-name` | Update database name |
| `--set <key=value>` | Set arbitrary config value |

**Output Structure:**
```json
{
  "name": "local",
  "message": "Profile 'local' updated successfully",
  "changes": ["database.host: new-host"]
}
```

**Artifact Type:** `Text`

---

### cloud profile delete

Delete a profile.

```bash
sp cloud profile delete <profile-name> --yes
sp cloud profile delete staging --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<name>` | Yes | Profile name |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "deleted": "staging",
  "message": "Profile 'staging' deleted successfully"
}
```

**Artifact Type:** `Text`

---

## Deployment Commands

### cloud deploy

Deploy to SystemPrompt Cloud.

```bash
sp cloud deploy
sp cloud deploy --profile staging
sp cloud deploy --skip-push
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--profile`, `-p` | Current profile | Profile to deploy |
| `--skip-push` | `false` | Skip Docker push (use existing image) |

**Output Structure:**
```json
{
  "deployed": true,
  "tenant_id": "tenant_abc123",
  "version": "v1.2.3",
  "url": "https://tenant_abc123.systemprompt.cloud",
  "message": "Deployment successful"
}
```

**Artifact Type:** `Text`

---

### cloud status

Check cloud deployment status.

```bash
sp cloud status
sp --json cloud status
```

**Output Structure:**
```json
{
  "tenant_id": "tenant_abc123",
  "status": "running",
  "version": "v1.2.3",
  "url": "https://tenant_abc123.systemprompt.cloud",
  "health": {
    "api": "healthy",
    "database": "healthy",
    "agents": "healthy"
  },
  "last_deployed": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### cloud restart

Restart tenant machine.

```bash
sp cloud restart --yes
sp cloud restart --tenant tenant_abc123 --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm restart |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--tenant` | Active tenant | Specific tenant ID |

**Output Structure:**
```json
{
  "tenant_id": "tenant_abc123",
  "status": "restarting",
  "message": "Restart initiated successfully"
}
```

**Artifact Type:** `Text`

---

## Sync Commands

### cloud sync

Sync between local and cloud environments.

```bash
sp cloud sync
sp cloud sync skills
sp cloud sync content
sp cloud sync admin-user
```

**Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `skills` | Sync skills configuration |
| `content` | Sync content to cloud |
| `admin-user` | Sync admin user |

**Output Structure:**
```json
{
  "synced": true,
  "items_synced": 5,
  "direction": "local-to-cloud",
  "message": "Sync completed successfully"
}
```

**Artifact Type:** `Text`

---

## Secrets Commands

### cloud secrets list

List configured secrets.

```bash
sp cloud secrets list
sp --json cloud secrets list
```

**Output Structure:**
```json
{
  "secrets": [
    {"name": "ANTHROPIC_API_KEY", "configured": true, "last_updated": "2024-01-15"},
    {"name": "OPENAI_API_KEY", "configured": false, "last_updated": null}
  ]
}
```

**Artifact Type:** `Table`

---

### cloud secrets set

Set a secret value.

```bash
sp cloud secrets set ANTHROPIC_API_KEY "sk-ant-..."
sp cloud secrets set --name OPENAI_API_KEY --value "sk-..."
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Secret name |
| `<value>` | Yes | Secret value |

**Output Structure:**
```json
{
  "name": "ANTHROPIC_API_KEY",
  "message": "Secret 'ANTHROPIC_API_KEY' set successfully"
}
```

**Artifact Type:** `Text`

---

## Dockerfile Generation

### cloud dockerfile

Generate Dockerfile based on discovered extensions.

```bash
sp cloud dockerfile
```

**Output:**
Prints a suggested Dockerfile to stdout based on:
- Discovered MCP servers
- Required runtime dependencies
- Extension configurations

**Artifact Type:** `Text`

---

## Complete Cloud Deployment Flow Example

This flow demonstrates deploying a project to SystemPrompt Cloud:

```bash
# Phase 1: Initialize project (if not done)
sp cloud init

# Phase 2: Authenticate
sp cloud auth login --email user@example.com --password "password"

# Phase 3: Verify authentication
sp --json cloud auth whoami

# Phase 4: Create or select tenant
sp cloud tenant create --name "My Project"
# or
sp cloud tenant select tenant_abc123

# Phase 5: Create production profile
sp cloud profile create \
  --name production \
  --environment production \
  --db-host prod-db.example.com

# Phase 6: Configure secrets
sp cloud secrets set ANTHROPIC_API_KEY "$ANTHROPIC_API_KEY"

# Phase 7: Deploy
sp cloud deploy --profile production

# Phase 8: Check status
sp --json cloud status

# Phase 9: Sync skills
sp cloud sync skills

# Phase 10: Verify deployment
curl https://tenant_abc123.systemprompt.cloud/health
```

---

## Multi-Environment Workflow Example

```bash
# Development (local)
export SYSTEMPROMPT_PROFILE=~/.systemprompt/profiles/local/profile.yaml
sp services start

# Staging deployment
sp cloud profile create --name staging --environment staging
sp cloud deploy --profile staging
sp --json cloud status

# Production deployment
sp cloud profile create --name production --environment production
sp cloud deploy --profile production
sp --json cloud status
```

---

## Error Handling

### Authentication Errors

```bash
sp cloud auth login --email wrong@example.com --password "wrong"
# Error: Authentication failed. Check your credentials.

sp cloud deploy
# Error: Not authenticated. Run 'cloud auth login' first.
```

### Tenant Errors

```bash
sp cloud tenant select nonexistent
# Error: Tenant 'nonexistent' not found

sp cloud deploy
# Error: No active tenant. Run 'cloud tenant select <id>' first.
```

### Profile Errors

```bash
sp cloud profile show nonexistent
# Error: Profile 'nonexistent' not found

sp cloud profile delete local --yes
# Error: Cannot delete active profile
```

### Deployment Errors

```bash
sp cloud deploy
# Error: Deployment failed. Check logs for details.

sp cloud status
# Error: Tenant is not deployed yet
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json cloud auth whoami | jq .

# Extract specific fields
sp --json cloud tenant | jq '.tenants[].id'
sp --json cloud profile | jq '.profiles[] | select(.active == true)'
sp --json cloud status | jq '.health'
sp --json cloud secrets list | jq '.secrets[] | select(.configured == true)'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag
- [x] Destructive operations (`restart`, `rotate`) require `--yes` / `-y`
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
