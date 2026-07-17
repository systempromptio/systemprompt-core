<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**CLI Reference**](https://github.com/systempromptio/systemprompt-core/tree/main/crates/entry/cli) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---


# Services CLI Commands

One binary supervises every service you run. The API server, each agent, and each MCP server start, stop, and restart under a single process tree, with one PID registry and one health view. This document is the complete reference for driving that tree from the CLI. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=~/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `infra services start` | Start API, agents, and MCP servers | `Text` | No |
| `infra services stop` | Stop running services gracefully | `Text` | No |
| `infra services restart` | Restart services | `Text` | Yes |
| `infra services status` | Show detailed service status | `Table` | No |
| `infra services cleanup` | Clean up orphaned processes | `Text` | No |
| `infra services serve` | Start API server (with agents/MCP) | `Text` | No |

---

## Core Commands

### services start

Start the API server, agents, and MCP servers. With no flags or target, all services start.

```bash
sp infra services start
sp infra services start --all
sp infra services start --api
sp infra services start --agents
sp infra services start --mcp
sp infra services start --skip-migrate
sp infra services start agent primary
sp infra services start mcp filesystem
```

**Target Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `agent <name>` | Start a single agent by name |
| `mcp <server_name>` | Start a single MCP server by name |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--all` | `false` | Start all services |
| `--api` | `false` | Start API server only |
| `--agents` | `false` | Start agents only |
| `--mcp` | `false` | Start MCP servers only |
| `--foreground` | `false` | Run in foreground (default behaviour) |
| `--skip-migrate` | `false` | Skip database migrations |
| `--kill-port-process` | `false` | Kill process using the port if occupied |

**Service Startup Order:**
1. Database migrations (unless skipped)
2. MCP servers
3. Agent processes
4. API server

**Output Structure:**
```json
{
  "started": true,
  "services": {
    "api": {"status": "running", "port": 8080, "pid": 12345},
    "agents": [
      {"name": "primary", "status": "running", "port": 8001, "pid": 12346}
    ],
    "mcp": [
      {"name": "filesystem", "status": "running", "port": 9001, "pid": 12347}
    ]
  },
  "message": "All services started successfully"
}
```

**Artifact Type:** `Text`

---

### services stop

Stop running services gracefully. With no flags or target, all services stop.

```bash
sp infra services stop
sp infra services stop --all
sp infra services stop --api
sp infra services stop --agents
sp infra services stop --mcp
sp infra services stop --force
sp infra services stop agent primary
sp infra services stop mcp filesystem --force
```

**Target Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `agent <name> [--force]` | Stop a single agent by name |
| `mcp <server_name> [--force]` | Stop a single MCP server by name |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--all` | `false` | Stop all services |
| `--api` | `false` | Stop API server only |
| `--agents` | `false` | Stop agents only |
| `--mcp` | `false` | Stop MCP servers only |
| `--force` | `false` | Force stop (SIGKILL) |

**Stop Order:**
1. API server
2. Agent processes
3. MCP servers

**Output Structure:**
```json
{
  "stopped": true,
  "services": {
    "api": {"status": "stopped"},
    "agents": [{"name": "primary", "status": "stopped"}],
    "mcp": [{"name": "filesystem", "status": "stopped"}]
  },
  "message": "All services stopped successfully"
}
```

**Artifact Type:** `Text`

---

### services restart

Restart a specific target, or restart only the services that failed.

```bash
sp infra services restart api
sp infra services restart agent primary
sp infra services restart mcp filesystem
sp infra services restart mcp filesystem --build
sp infra services restart --failed
sp infra services restart --agents
sp infra services restart --mcp
```

**Target Subcommands:**
| Subcommand | Description |
|------------|-------------|
| `api` | Restart the API server |
| `agent <agent>` | Restart a single agent by name |
| `mcp <server_name> [--build]` | Restart a single MCP server; `--build` rebuilds the binary first |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--failed` | `false` | Restart only failed services |
| `--agents` | `false` | Restart all agents |
| `--mcp` | `false` | Restart all MCP servers |

**Output Structure:**
```json
{
  "restarted": ["primary"],
  "message": "Service 'primary' restarted successfully"
}
```

**Artifact Type:** `Text`

---

### services status

Show detailed service status.

```bash
sp infra services status
sp --json infra services status
sp infra services status --detailed
sp infra services status --health
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--detailed` | `false` | Show detailed information |
| `--json` | `false` | Output as JSON |
| `--health` | `false` | Include health check results |

**Output Structure:**
```json
{
  "api": {
    "status": "running",
    "port": 8080,
    "pid": 12345,
    "uptime_seconds": 3600,
    "health": "healthy"
  },
  "agents": [
    {
      "name": "primary",
      "status": "running",
      "port": 8001,
      "pid": 12346,
      "enabled": true,
      "health": "healthy"
    }
  ],
  "mcp_servers": [
    {
      "name": "filesystem",
      "status": "running",
      "port": 9001,
      "pid": 12347,
      "enabled": true,
      "health": "healthy"
    }
  ],
  "summary": {
    "total_services": 5,
    "running": 5,
    "stopped": 0,
    "failed": 0
  }
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `type`, `status`, `port`, `pid`, `health`

---

### services cleanup

Clean up orphaned processes and stale entries.

```bash
sp infra services cleanup              # Interactive mode prompts for confirmation
sp infra services cleanup --yes        # Skip confirmation
sp infra services cleanup --dry-run    # Preview what would be cleaned
sp --json infra services cleanup --yes
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--yes` / `-y` | `false` | Skip confirmation prompt |
| `--dry-run` | `false` | Preview cleanup without executing |

**Cleanup Actions:**
- Terminate orphaned processes
- Remove stale PID files
- Clean up temporary files
- Reset service state

**Output Structure:**
```json
{
  "cleaned": true,
  "processes_killed": 2,
  "pid_files_removed": 3,
  "temp_files_removed": 5,
  "message": "Cleanup completed successfully"
}
```

**Artifact Type:** `Text`

---

### services serve

Start the API server, which automatically starts agents and MCP servers.

```bash
sp infra services serve
sp infra services serve --foreground
sp infra services serve --kill-port-process
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--foreground` | `false` | Run in foreground mode |
| `--kill-port-process` | `false` | Kill process using the port if occupied |

**Output Structure:**
```json
{
  "started": true,
  "api_port": 8080,
  "url": "http://localhost:8080",
  "message": "API server started at http://localhost:8080"
}
```

**Artifact Type:** `Text`

---

## Complete Services Workflow Example

This flow demonstrates managing services:

```bash
# Phase 1: Check current status
sp --json infra services status

# Phase 2: Start all services
sp infra services start

# Phase 3: Verify status
sp --json infra services status --health

# Phase 4: Check specific service
sp --json infra services status | jq '.agents[] | select(.name == "primary")'

# Phase 5: Restart failed services
sp infra services restart --failed

# Phase 6: Stop all services
sp infra services stop

# Phase 7: Cleanup
sp infra services cleanup
```

---

## Development Workflow

```bash
# Start faster by skipping migrations
sp infra services start --skip-migrate

# Restart a specific agent after code changes
sp infra services restart agent primary

# Rebuild and restart an MCP server after code changes
sp infra services restart mcp filesystem --build

# Force restart if hanging
sp infra services stop --force
sp infra services start
```

---

## Production Workflow

```bash
# Full startup with migrations
sp infra services start

# Health check
sp --json infra services status --health

# Graceful restart
sp infra services stop
sp infra services start

# Monitor status
watch -n 5 'sp --json infra services status | jq .summary'
```

---

## Service Configuration

Services are configured in `services.yaml`:

```yaml
api:
  port: 8080
  host: "0.0.0.0"

agents:
  primary:
    enabled: true
    port: 8001
    provider: anthropic
    model: claude-sonnet-4-6-20250610

mcp_servers:
  filesystem:
    enabled: true
    port: 9001
    command: "./target/debug/mcp-filesystem"
```

---

## Error Handling

### Port Already in Use

```bash
sp infra services start
# Error: Port 8080 is already in use. Use --kill-port-process to terminate existing process.

sp infra services serve --kill-port-process
# Killed process on port 8080, starting server...
```

### Service Already Running

```bash
sp infra services start --api
# Warning: API server is already running on port 8080

sp infra services status
# Shows current running services
```

### Database Connection Error

```bash
sp infra services start
# Error: Failed to connect to database. Check your profile configuration.
```

### Target Required

```bash
sp infra services restart
# Error: Must specify target (api, agent, mcp) or use --failed flag
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json infra services status | jq .

# Extract specific fields
sp --json infra services status | jq '.api.port'
sp --json infra services status | jq '.agents[].name'
sp --json infra services status | jq '.summary'

# Check running services
sp --json infra services status | jq '.agents[] | select(.status == "running")'

# Get health status
sp --json infra services status --health | jq '.agents[] | {name, health}'
```

---

## Compliance Checklist

- [x] Group dispatch accepts `ctx: &CommandContext`; leaf `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Graceful shutdown handling


---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
