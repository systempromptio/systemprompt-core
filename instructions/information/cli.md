# systemprompt.io CLI User Guide

Complete reference for the systemprompt.io command-line interface. The CLI is the primary interface for managing the platform, including services, agents, databases, content, analytics, and cloud deployments.

---

## Quick Start

```bash
# Initial setup
systemprompt admin setup

# Start all services
systemprompt infra services start --all

# Check status
systemprompt infra services status
```

---

## Command Structure

The CLI is organized into clear domains:

| Domain | Description |
|--------|-------------|
| `core` | Core platform operations (content, files, contexts, skills) |
| `infra` | Infrastructure management (services, db, jobs, logs, system) |
| `admin` | Administration (users, agents, config, setup, session) |
| `cloud` | Cloud deployment, sync, and setup |
| `analytics` | Analytics and metrics reporting |
| `web` | Web service configuration management |
| `plugins` | Plugins, extensions, and MCP server management |
| `build` | Build MCP extensions |

---

## Global Options

These options apply to all commands:

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Increase output verbosity |
| `-q, --quiet` | Suppress output |
| `--debug` | Enable debug logging |
| `--json` | Output as JSON |
| `--yaml` | Output as YAML |
| `--no-color` | Disable colored output |
| `--non-interactive` | Non-interactive mode |
| `--database-url <URL>` | Direct database URL (bypasses profile) |

---

## Core Domain

Core platform operations for content, files, contexts, and skills.

### core content

Content management and analytics.

| Subcommand | Description |
|------------|-------------|
| `list` | List content with pagination |
| `show <ID>` | Show content details |
| `search <QUERY>` | Search content |
| `ingest <DIR>` | Ingest markdown files from directory |
| `delete <ID>` | Delete content by ID |
| `delete-source <SOURCE>` | Delete all content from a source |
| `popular` | Get popular content |
| `verify <ID>` | Verify content is published and accessible |
| `status <SOURCE>` | Show content health status for a source |
| `link` | Link generation and management |
| `analytics` | Content analytics |
| `publish` | Publish static content (ingest, prerender, sitemap) |
| `files` | Content-file linking operations |

```bash
# List content
systemprompt core content list

# Search
systemprompt core content search "getting started"

# Ingest markdown files
systemprompt core content ingest ./docs

# Content-file operations
systemprompt core content files link content_123 file_456
systemprompt core content files list content_123
```

### core files

File management and uploads.

| Subcommand | Description |
|------------|-------------|
| `list` | List files with pagination |
| `show <ID>` | Show detailed file information |
| `upload <PATH>` | Upload a file from local filesystem |
| `delete <ID>` | Delete a file |
| `validate <PATH>` | Validate a file before upload |
| `config` | Show file upload configuration |
| `search <PATTERN>` | Search files by path pattern |
| `stats` | Show file storage statistics |
| `ai` | AI-generated images operations |

```bash
# List files
systemprompt core files list --limit 50

# Upload file
systemprompt core files upload ./image.png

# Search files
systemprompt core files search "*.png"

# AI-generated images
systemprompt core files ai list
systemprompt core files ai count
```

### core contexts

Manage conversation contexts and execution environments.

| Subcommand | Description |
|------------|-------------|
| `list` | List all contexts with stats |
| `show <ID>` | Show context details |
| `create <NAME>` | Create a new context |
| `edit <NAME>` | Rename a context |
| `delete <NAME>` | Delete a context |
| `use <ID>` | Set session's active context |
| `new <NAME>` | Create a new context and set it as active |

```bash
# List contexts
systemprompt core contexts list

# Create and switch to new context
systemprompt core contexts new "feature-development"

# Switch to existing context
systemprompt core contexts use ctx_abc123
```

### core skills

Skill management and database sync.

| Subcommand | Description |
|------------|-------------|
| `list` | List configured skills |
| `create <NAME>` | Create new skill |
| `edit <NAME>` | Edit skill configuration |
| `delete <NAME>` | Delete a skill |
| `status` | Show database sync status |
| `sync` | Sync skills between disk and database |

```bash
# List skills
systemprompt core skills list

# Create skill
systemprompt core skills create my-skill

# Sync with database
systemprompt core skills sync
```

---

## Infrastructure Domain

Infrastructure management for services, database, jobs, logs, and system.

### infra services

Manage API servers, agents, and MCP servers lifecycle.

#### services start

Start services.

| Option | Description |
|--------|-------------|
| `--all` | Start all services |
| `--api` | Start API server only |
| `--agents` | Start agents only |
| `--mcp` | Start MCP servers only |
| `--foreground` | Run in foreground (default) |
| `--skip-web` | Skip web asset build |
| `--skip-migrate` | Skip database migrations |

```bash
# Start everything
systemprompt infra services start --all

# Start only the API server
systemprompt infra services start --api

# Start without migrations
systemprompt infra services start --all --skip-migrate
```

#### services stop

Stop running services gracefully.

| Option | Description |
|--------|-------------|
| `--all` | Stop all services |
| `--api` | Stop API server only |
| `--agents` | Stop agents only |
| `--mcp` | Stop MCP servers only |
| `--force` | Force stop (SIGKILL) |

```bash
# Stop everything
systemprompt infra services stop --all

# Force stop all
systemprompt infra services stop --all --force
```

#### services restart

Restart services.

| Subcommand | Description |
|------------|-------------|
| `api` | Restart API server |
| `agent <AGENT_ID>` | Restart specific agent |
| `mcp <SERVER_NAME>` | Restart specific MCP server |

| Option | Description |
|--------|-------------|
| `--failed` | Restart only failed services |
| `--agents` | Restart all agents |
| `--mcp` | Restart all MCP servers |
| `--build` | Rebuild before restart (MCP only) |

```bash
# Restart API server
systemprompt infra services restart api

# Restart a specific agent
systemprompt infra services restart agent my-agent

# Restart MCP server with rebuild
systemprompt infra services restart mcp my-server --build
```

#### services status

Show detailed service status.

| Option | Description |
|--------|-------------|
| `--detailed` | Show detailed information |
| `--json` | Output as JSON |
| `--health` | Include health check results |

```bash
# Basic status
systemprompt infra services status

# Detailed with health checks
systemprompt infra services status --detailed --health
```

#### services cleanup

Clean up orphaned processes and stale entries.

| Option | Description |
|--------|-------------|
| `-y, --yes` | Skip confirmation prompt |
| `--dry-run` | Preview cleanup without executing |

```bash
# Preview cleanup
systemprompt infra services cleanup --dry-run

# Execute cleanup
systemprompt infra services cleanup -y
```

#### services serve

Start the API server (automatically starts agents and MCP servers).

| Option | Description |
|--------|-------------|
| `--foreground` | Run in foreground mode |
| `--kill-port-process` | Kill process using the port if occupied |

```bash
systemprompt infra services serve --foreground
```

### infra db

Database operations and administration.

| Subcommand | Description |
|------------|-------------|
| `query <SQL>` | Execute read-only SQL query |
| `execute <SQL>` | Execute write operations |
| `tables` | List all tables with row counts |
| `describe <TABLE>` | Describe table schema |
| `info` | Show database information |
| `migrate` | Run database migrations |
| `assign-admin <EMAIL>` | Assign admin role to user |
| `status` | Show database connection status |
| `validate` | Validate database schema |
| `count <TABLE>` | Get row count for a table |
| `indexes` | List all indexes |
| `size` | Show database and table sizes |

```bash
# Check connection
systemprompt infra db status

# Run migrations
systemprompt infra db migrate

# Explore schema
systemprompt infra db tables
systemprompt infra db describe users

# Query data
systemprompt infra db query "SELECT * FROM users LIMIT 10"
```

### infra jobs

Manage background jobs and scheduling.

| Subcommand | Description |
|------------|-------------|
| `list` | List available jobs |
| `show <NAME>` | Show detailed job information |
| `run <NAME>` | Run a scheduled job manually |
| `history` | View job execution history |
| `enable <NAME>` | Enable a job |
| `disable <NAME>` | Disable a job |
| `cleanup-sessions` | Clean up inactive sessions |
| `log-cleanup` | Clean up old log entries |

```bash
# List jobs
systemprompt infra jobs list

# Run specific job
systemprompt infra jobs run cleanup_sessions

# View job history
systemprompt infra jobs history --status failed --limit 50

# Enable/disable jobs
systemprompt infra jobs enable my_job
systemprompt infra jobs disable my_job
```

### infra logs

View, search, and analyze logs.

| Subcommand | Description |
|------------|-------------|
| `view` | View log entries |
| `search <PATTERN>` | Search logs by pattern |
| `stream` | Stream logs in real-time |
| `export` | Export logs to file |
| `cleanup` | Clean up old log entries |
| `delete` | Delete all log entries |
| `summary` | Show logs summary statistics |
| `show <ID>` | Show log entry or trace details |
| `trace` | Debug execution traces |
| `request` | Inspect AI requests |
| `tools` | List MCP tool executions |
| `audit <ID>` | Full audit of an AI request |

```bash
# Recent logs
systemprompt infra logs view --tail 50

# Error logs from last hour
systemprompt infra logs view --level error --since 1h

# Stream in real-time
systemprompt infra logs stream --level error --module agent

# Export for analysis
systemprompt infra logs export --format json --since 24h -o logs.json
```

### infra system

System-level authentication and session operations.

| Subcommand | Description |
|------------|-------------|
| `login` | Create a session and get an authentication token |

```bash
systemprompt admin session login
```

---

## Admin Domain

Administration for users, agents, configuration, setup, and sessions.

### admin users

User management and IP banning.

| Subcommand | Description |
|------------|-------------|
| `list` | List users with pagination |
| `show <ID>` | Show detailed user information |
| `search <QUERY>` | Search users |
| `create` | Create a new user |
| `update <ID>` | Update user fields |
| `delete <ID>` | Delete a user |
| `count` | Get total user count |
| `export` | Export users to JSON |
| `stats` | Show user statistics |
| `merge <SRC> <DST>` | Merge source user into target |
| `bulk` | Bulk operations (delete, update) |
| `role` | Role management (assign, promote, demote) |
| `session` | Session management (list, end, cleanup) |
| `ban` | IP ban management (list, add, remove, check) |

```bash
# List users
systemprompt admin users list --limit 50

# Search for user
systemprompt admin users search "john@example.com"

# Role management
systemprompt admin users role promote user_123
systemprompt admin users role assign user_123 admin,editor

# Session management
systemprompt admin users session list user_123
systemprompt admin users session cleanup --hours 24

# IP ban management
systemprompt admin users ban add 192.168.1.100 --duration 1440 --reason "Abuse"
systemprompt admin users ban list
```

### admin agents

Create, configure, and manage AI agents.

| Subcommand | Description |
|------------|-------------|
| `list [FILTER]` | List configured agents |
| `show <NAME>` | Display agent configuration |
| `validate [FILTER]` | Check agent configs for errors |
| `create <NAME>` | Create new agent |
| `edit <NAME>` | Edit agent configuration |
| `delete <NAME>` | Delete an agent |
| `status <NAME>` | Show agent process status |
| `logs <NAME>` | View agent logs |
| `registry` | Get running agents from gateway registry |
| `message <NAME>` | Send a message to an agent via A2A |
| `task <NAME>` | Get task details from an agent |
| `tools <NAME>` | List MCP tools available to an agent |
| `run` | Run an agent server (internal use) |

```bash
# List all agents
systemprompt admin agents list

# Create a new agent
systemprompt admin agents create my-assistant

# Edit agent configuration
systemprompt admin agents edit my-assistant

# Show agent status
systemprompt admin agents status my-assistant

# Send message to agent via A2A
systemprompt admin agents message my-assistant "Hello, what can you do?"
```

### admin config

Configuration management and rate limits.

| Subcommand | Description |
|------------|-------------|
| `show` | Show configuration overview |
| `rate-limits` | Rate limit configuration |
| `server` | Server configuration |
| `runtime` | Runtime configuration |
| `security` | Security configuration |
| `paths` | Paths configuration |

```bash
# Show overview
systemprompt admin config show

# View rate limits
systemprompt admin config rate-limits

# Server settings
systemprompt admin config server
```

### admin setup

Interactive setup wizard for local development environment.

```bash
systemprompt admin setup
```

Guides you through initial configuration including profile creation, database setup, and environment configuration.

### admin session

Manage CLI sessions and profile switching.

| Subcommand | Description |
|------------|-------------|
| `show` | Show current session and routing info |
| `switch <PROFILE>` | Switch to a different profile |
| `list` | List available profiles |

```bash
# Show current session
systemprompt admin session show

# Switch to production profile
systemprompt admin session switch production

# List all profiles
systemprompt admin session list
```

---

## Cloud Domain

Cloud deployment, sync, and setup.

### cloud auth

Authentication management.

| Subcommand | Description |
|------------|-------------|
| `login [ENV]` | Authenticate via OAuth |
| `logout` | Clear saved credentials |
| `whoami` | Show current user and token status |

```bash
# Login to cloud
systemprompt cloud auth login

# Check authentication
systemprompt cloud auth whoami

# Logout
systemprompt cloud auth logout -y
```

### cloud init

Initialize project structure.

| Option | Description |
|--------|-------------|
| `--force` | Force initialization |

```bash
systemprompt cloud init
```

### cloud tenant

Manage tenants (local or cloud).

| Subcommand | Description |
|------------|-------------|
| `create` | Create a new tenant |
| `list` | List all tenants |
| `show [ID]` | Show tenant details |
| `delete [ID]` | Delete a tenant |
| `edit [ID]` | Edit tenant configuration |
| `rotate-credentials [ID]` | Rotate database credentials |
| `rotate-sync-token [ID]` | Rotate sync token |

```bash
# Create tenant
systemprompt cloud tenant create --region iad

# List tenants
systemprompt cloud tenant list

# Rotate credentials
systemprompt cloud tenant rotate-credentials my-tenant -y
```

### cloud profile

Manage profiles.

| Subcommand | Description |
|------------|-------------|
| `create <NAME>` | Create a new profile |
| `list` | List all profiles |
| `show [NAME]` | Show profile configuration |
| `delete <NAME>` | Delete a profile |
| `edit [NAME]` | Edit profile configuration |

```bash
# Create profile
systemprompt cloud profile create production

# Show profile with filter
systemprompt cloud profile show production --filter agents --json
```

### cloud deploy

Deploy to systemprompt.io Cloud.

| Option | Description |
|--------|-------------|
| `--skip-push` | Skip push step |
| `-p, --profile <NAME>` | Profile name to deploy |

```bash
systemprompt cloud deploy --profile production
```

### cloud sync

Sync between local and cloud environments.

| Subcommand | Description |
|------------|-------------|
| `down` | Sync from cloud to local |
| `up` | Sync from local to cloud |

```bash
# Pull from cloud
systemprompt cloud sync down

# Push to cloud
systemprompt cloud sync up
```

### cloud db

Cloud database operations.

| Subcommand | Description |
|------------|-------------|
| `migrate` | Run migrations on cloud database |
| `query` | Execute queries on cloud database |
| `admin` | Admin operations |

```bash
systemprompt cloud db migrate
systemprompt cloud db query "SELECT COUNT(*) FROM users"
```

---

## Analytics Domain

Analytics and metrics reporting.

| Subcommand | Description |
|------------|-------------|
| `overview` | Dashboard overview of all analytics |
| `conversations` | Conversation analytics (stats, trends, list) |
| `agents` | Agent performance analytics (stats, list, trends, show) |
| `tools` | Tool usage analytics (stats, list, trends, show) |
| `requests` | AI request analytics (stats, trends, models) |
| `sessions` | Session analytics (stats, trends, live) |
| `content` | Content performance analytics (stats, trends, popular) |
| `traffic` | Traffic analytics (sources, geo, devices, bots) |
| `costs` | Cost analytics (summary, trends, breakdown) |

```bash
# Overview dashboard
systemprompt analytics overview

# Agent performance
systemprompt analytics agents stats
systemprompt analytics agents show my-agent

# Cost analysis
systemprompt analytics costs summary
systemprompt analytics costs breakdown
```

---

## Web Domain

Web service configuration management.

| Subcommand | Description |
|------------|-------------|
| `content-types` | Manage content types |
| `templates` | Manage templates |
| `assets` | List and inspect assets |
| `sitemap` | Sitemap operations |
| `validate` | Validate web configuration |

```bash
# List content types
systemprompt web content-types

# Validate configuration
systemprompt web validate

# Generate sitemap
systemprompt web sitemap
```

---

## Plugins Domain

Plugins, extensions, and MCP server management.

| Subcommand | Description |
|------------|-------------|
| `list` | List all discovered extensions |
| `show <NAME>` | Show detailed extension information |
| `run <EXT> [ARGS...]` | Run a CLI extension command |
| `validate` | Validate extension dependencies |
| `config` | Show extension configuration |
| `capabilities` | List capabilities across all extensions |
| `mcp` | MCP server management |

```bash
# List extensions
systemprompt plugins list

# Show extension details
systemprompt plugins show my-extension

# Run CLI extension command
systemprompt plugins run my-cli homepage regenerate
systemprompt --json plugins run my-cli status

# MCP server management
systemprompt plugins mcp list
systemprompt plugins mcp status
systemprompt plugins mcp tools
```

---

## Build Domain

Build MCP extensions and web assets.

| Subcommand | Description |
|------------|-------------|
| `core` | Build Rust workspace (systemprompt-core) |
| `web` | Build web frontend |
| `mcp` | Build MCP extensions |

```bash
# Build core
systemprompt build core

# Build web assets
systemprompt build web

# Build MCP extensions
systemprompt build mcp
```

---

## Command Requirements

Commands have varying initialization requirements:

| Requirement | Description | Examples |
|-------------|-------------|----------|
| **FULL** | Requires profile, secrets, and paths | Most commands |
| **PROFILE_AND_SECRETS** | Requires profile and secrets | infra system |
| **PROFILE_ONLY** | Requires profile only | build, plugins |
| **NONE** | No requirements | admin setup, admin session, cloud auth |

---

## Common Workflows

### Initial Setup

```bash
# Run setup wizard
systemprompt admin setup

# Initialize cloud project (optional)
systemprompt cloud init

# Start services
systemprompt infra services start --all

# Check status
systemprompt infra services status --health
```

### Development Workflow

```bash
# Start services in foreground
systemprompt infra services start --all --foreground

# View logs in real-time
systemprompt infra logs stream --level info

# Check agent status
systemprompt admin agents status my-agent

# Restart specific agent after changes
systemprompt infra services restart agent my-agent
```

### Cloud Deployment

```bash
# Authenticate
systemprompt cloud auth login

# Create tenant
systemprompt cloud tenant create --region iad

# Create production profile
systemprompt cloud profile create production

# Configure profile
systemprompt cloud profile edit production

# Deploy
systemprompt cloud deploy --profile production

# Check status
systemprompt cloud status
```

### Database Operations

```bash
# Check connection
systemprompt infra db status

# Run migrations
systemprompt infra db migrate

# Explore schema
systemprompt infra db tables
systemprompt infra db describe users

# Query data
systemprompt infra db query "SELECT * FROM users WHERE status = 'active' LIMIT 10"
```

### Monitoring and Debugging

```bash
# Overview dashboard
systemprompt analytics overview

# Check for errors
systemprompt infra logs view --level error --since 1h

# Trace specific request
systemprompt infra logs audit request_id --full

# Agent performance
systemprompt analytics agents show my-agent

# Cost analysis
systemprompt analytics costs breakdown
```

### User Management

```bash
# List users
systemprompt admin users list --limit 100

# Search for user
systemprompt admin users search "john@example.com"

# Promote to admin
systemprompt admin users role promote user_123

# Clean up old sessions
systemprompt admin users session cleanup --hours 48
```

---

## CLI Extension Development

For extending the CLI with custom commands, see the extension architecture:

### manifest.yaml

```yaml
extension:
  type: cli
  name: My CLI Extension
  binary: my-cli
  description: Custom CLI commands
  enabled: true
  commands:
    - name: homepage
      description: Homepage management
    - name: content
      description: Content operations
```

### Usage

```bash
systemprompt plugins list
systemprompt plugins run my-cli homepage regenerate
systemprompt plugins run my-cli content generate --type blog
systemprompt --json plugins run my-cli homepage status
```

### Environment Variables

Core propagates to CLI extension binaries:

| Variable | Purpose |
|----------|---------|
| `SYSTEMPROMPT_PROFILE` | Profile path |
| `JWT_SECRET` | JWT signing secret |
| `DATABASE_URL` | Database connection |

---

## Troubleshooting

### Services won't start

```bash
# Check for port conflicts
systemprompt infra services serve --kill-port-process

# Clean up orphaned processes
systemprompt infra services cleanup --dry-run
systemprompt infra services cleanup -y
```

### Database connection issues

```bash
# Check status
systemprompt infra db status

# Validate schema
systemprompt infra db validate

# Check database info
systemprompt infra db info
```

### Cloud deployment issues

```bash
# Check authentication
systemprompt cloud auth whoami

# Check deployment status
systemprompt cloud status

# Restart tenant
systemprompt cloud restart --tenant my-tenant -y
```

### Log analysis

```bash
# Search for specific errors
systemprompt infra logs search "connection refused" --since 1h

# Export for analysis
systemprompt infra logs export --format json --since 24h -o debug.json

# Summary statistics
systemprompt infra logs summary --since 1h
```
