# Setup CLI Commands

This document provides complete documentation for AI agents to use the setup CLI commands. All commands support non-interactive mode for automation.

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
| `admin setup` | Interactive/non-interactive environment setup wizard | `Text` | No |

---

## Setup Command

### setup

Initialize a new environment with PostgreSQL database and configuration.

```bash
# Interactive mode (prompts for values)
./target/debug/systemprompt setup

# Non-interactive mode with all required flags
sp admin setup \
  --environment dev \
  --db-host localhost \
  --db-port 5432 \
  --db-user systemprompt_dev \
  --db-password "secure_password" \
  --db-name systemprompt_dev \
  --anthropic-key "sk-ant-..." \
  --migrate

# Using Docker for PostgreSQL
sp admin setup \
  --environment dev \
  --docker \
  --anthropic-key "sk-ant-..." \
  --migrate

# Minimal setup (uses defaults)
sp admin setup --environment dev --no-migrate
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--environment`, `-e` | Yes | Target environment name (e.g., dev, staging, prod) |

**Database Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--docker` | `false` | Use Docker for PostgreSQL instead of existing installation |
| `--db-host` | `localhost` | PostgreSQL host |
| `--db-port` | `5432` | PostgreSQL port |
| `--db-user` | `systemprompt_<env>` | PostgreSQL user |
| `--db-password` | Auto-generated | PostgreSQL password |
| `--db-name` | `systemprompt_<env>` | PostgreSQL database name |

**AI Provider Flags:**
| Flag | Environment Variable | Description |
|------|---------------------|-------------|
| `--anthropic-key` | `ANTHROPIC_API_KEY` | Anthropic (Claude) API key |
| `--openai-key` | `OPENAI_API_KEY` | OpenAI (GPT) API key |
| `--gemini-key` | `GEMINI_API_KEY` | Google AI (Gemini) API key |
| `--github-token` | `GITHUB_TOKEN` | GitHub token (optional) |

**Migration Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--migrate` | `false` | Run database migrations after setup |
| `--no-migrate` | `false` | Skip migrations (default for non-interactive) |

**Validation Rules:**
- Environment name: Required, used to generate default database credentials
- At least one AI provider key is recommended for full functionality
- Port must be valid (1-65535)

---

## Setup Process

The setup wizard performs these steps:

### 1. Environment Configuration
- Creates profile directory structure
- Generates environment-specific configuration files

### 2. Database Setup
- **Docker mode**: Creates Docker container for PostgreSQL
- **Existing mode**: Connects to existing PostgreSQL instance
- Creates database user and database if they don't exist
- Tests database connection

### 3. Secrets Configuration
- Stores AI provider API keys securely
- Encrypts sensitive credentials
- Generates secure random values for missing optional secrets

### 4. Profile Creation
- Creates `profile.yaml` with all configuration
- Sets up service definitions for agents and MCP servers

### 5. Database Migration (Optional)
- Runs all schema migrations
- Creates required tables
- Initializes default data

---

## Output Structure

```json
{
  "environment": "dev",
  "profile_path": "/var/www/html/tyingshoelaces/.systemprompt/profiles/dev/profile.yaml",
  "database": {
    "host": "localhost",
    "port": 5432,
    "name": "systemprompt_dev",
    "user": "systemprompt_dev",
    "connection_status": "connected"
  },
  "secrets_configured": {
    "anthropic": true,
    "openai": false,
    "gemini": false,
    "github": false
  },
  "migrations_run": true,
  "message": "Environment 'dev' setup completed successfully"
}
```

**Artifact Type:** `Text`

---

## Complete Setup Flow Example

This flow demonstrates setting up a new development environment:

```bash
# Phase 1: Setup with Docker PostgreSQL
sp admin setup \
  --environment dev \
  --docker \
  --anthropic-key "$ANTHROPIC_API_KEY" \
  --migrate

# Phase 2: Verify database connection
sp infra db status

# Phase 3: Verify profile was created
cat /var/www/html/tyingshoelaces/.systemprompt/profiles/dev/profile.yaml

# Phase 4: Test with the new profile
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/dev/profile.yaml
sp admin agents list
```

---

## Setup with Existing PostgreSQL

```bash
# Phase 1: Create database and user manually (if needed)
psql -h localhost -U postgres <<EOF
CREATE USER systemprompt_dev WITH PASSWORD 'your_password';
CREATE DATABASE systemprompt_dev OWNER systemprompt_dev;
GRANT ALL PRIVILEGES ON DATABASE systemprompt_dev TO systemprompt_dev;
EOF

# Phase 2: Run setup pointing to existing database
sp admin setup \
  --environment dev \
  --db-host localhost \
  --db-port 5432 \
  --db-user systemprompt_dev \
  --db-password "your_password" \
  --db-name systemprompt_dev \
  --anthropic-key "$ANTHROPIC_API_KEY" \
  --migrate

# Phase 3: Verify connection
sp infra db status
```

---

## Setup with Docker PostgreSQL

```bash
# Phase 1: Setup with Docker (creates container automatically)
sp admin setup \
  --environment dev \
  --docker \
  --anthropic-key "$ANTHROPIC_API_KEY" \
  --migrate

# Phase 2: Verify Docker container is running
docker ps | grep systemprompt

# Phase 3: Access database
docker exec -it systemprompt-postgres-dev psql -U systemprompt_dev
```

---

## Multiple Environment Setup

```bash
# Development environment
sp admin setup \
  --environment dev \
  --docker \
  --anthropic-key "$ANTHROPIC_API_KEY" \
  --migrate

# Staging environment
sp admin setup \
  --environment staging \
  --db-host staging-db.example.com \
  --db-port 5432 \
  --db-user systemprompt_staging \
  --db-password "$STAGING_DB_PASSWORD" \
  --db-name systemprompt_staging \
  --anthropic-key "$ANTHROPIC_API_KEY" \
  --migrate

# Production environment
sp admin setup \
  --environment prod \
  --db-host prod-db.example.com \
  --db-port 5432 \
  --db-user systemprompt_prod \
  --db-password "$PROD_DB_PASSWORD" \
  --db-name systemprompt_prod \
  --anthropic-key "$ANTHROPIC_API_KEY" \
  --migrate
```

---

## Error Handling

### Missing Required Flags

```bash
sp admin setup
# Error: --environment is required in non-interactive mode
```

### Database Connection Errors

```bash
sp admin setup --environment dev --db-host invalid-host --no-migrate
# Error: Failed to connect to PostgreSQL at invalid-host:5432
```

### Docker Errors

```bash
sp admin setup --environment dev --docker --no-migrate
# Error: Docker is not running. Please start Docker and try again.
```

### Invalid API Keys

```bash
sp admin setup --environment dev --anthropic-key "invalid" --no-migrate
# Warning: Anthropic API key format appears invalid (should start with sk-ant-)
```

### Migration Conflicts

```bash
sp admin setup --environment dev --migrate --no-migrate
# Error: Cannot specify both --migrate and --no-migrate
```

---

## Configuration Files Created

After setup, the following files are created:

```
~/.systemprompt/profiles/<environment>/
├── profile.yaml          # Main profile configuration
├── secrets.yaml          # Encrypted secrets (API keys)
└── services/
    ├── agents/           # Agent configurations
    └── mcp/              # MCP server configurations
```

### profile.yaml Structure

```yaml
environment: dev
database:
  host: localhost
  port: 5432
  name: systemprompt_dev
  user: systemprompt_dev
services_path: ./services
logs_path: ./logs
```

---

## Post-Setup Verification

```bash
# Set profile for subsequent commands
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/dev/profile.yaml

# Verify database connection
sp infra db status

# Verify database tables
sp infra db tables

# List configured agents
sp admin agents list

# List MCP servers
sp plugins mcp list

# Run a test query
sp infra db query "SELECT version()"
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] `resolve_input` pattern used for interactive/non-interactive selection
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
- [x] Environment variables supported as fallback for API keys
