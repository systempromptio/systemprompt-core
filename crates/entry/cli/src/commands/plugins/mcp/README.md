<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# MCP CLI Commands

This document provides complete documentation for AI agents to use the MCP (Model Context Protocol) CLI commands. All commands support non-interactive mode for automation.

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
| `plugins mcp list` | List MCP server configurations | `Table` | No |
| `plugins mcp status` | Show running MCP server status | `Table` | No |
| `plugins mcp validate <name>` | Validate MCP server connection | `Card` | Yes |
| `plugins mcp validate --all` | Validate all MCP servers | `Card` | Yes |
| `plugins mcp logs <name>` | View MCP server logs | `Text` | No |
| `plugins mcp list-packages` | List package names for build | `List` | No |

---

## Core Commands

### mcp list

List all configured MCP servers from the services configuration.

```bash
sp plugins mcp list
sp --json mcp list
sp plugins mcp list --enabled
sp plugins mcp list --disabled
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--enabled` | Show only enabled servers |
| `--disabled` | Show only disabled servers |

**Output Structure:**
```json
{
  "servers": [
    {
      "name": "filesystem",
      "port": 9001,
      "enabled": true,
      "status": "ready",
      "debug_binary": "/path/to/target/debug/mcp-filesystem",
      "debug_created_at": "2024-01-15 10:30:00",
      "release_binary": "/path/to/target/release/mcp-filesystem",
      "release_created_at": "2024-01-15 10:00:00"
    }
  ]
}
```

**Status Values:**
- `ready` - Both debug and release binaries exist
- `debug-only` - Only debug binary exists
- `release-only` - Only release binary exists
- `not-built` - No binaries exist
- `disabled` - Server is disabled in configuration

**Artifact Type:** `Table`
**Columns:** `name`, `port`, `enabled`, `status`, `debug_binary`, `release_binary`

---

### mcp status

Show running MCP server status with binary information.

```bash
sp plugins mcp status
sp --json mcp status
sp plugins mcp status --detailed
sp plugins mcp status --server content-manager
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--detailed`, `-d` | `false` | Show full binary paths instead of "exists" |
| `--server` | All | Filter to specific server by name |

**Output Structure:**
```json
{
  "servers": [
    {
      "name": "filesystem",
      "enabled": true,
      "running": true,
      "pid": 12345,
      "port": 9001,
      "binary": "mcp-filesystem",
      "release_binary": "exists",
      "debug_binary": "exists"
    }
  ],
  "summary": {
    "total": 3,
    "enabled": 2,
    "running": 2
  }
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `port`, `enabled`, `running`, `pid`, `release_binary`, `debug_binary`

---

### mcp validate

Validate MCP server connection and capabilities. Returns rich validation data including tools count, latency, and server info.

```bash
sp plugins mcp validate <server-name>
sp --json mcp validate filesystem
sp plugins mcp validate database --timeout 30
sp plugins mcp validate --all
sp plugins mcp validate --all --timeout 5
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes* | MCP server name to validate (*not required if --all is used) |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--timeout` | `10` | Connection timeout in seconds |
| `--all` | `false` | Validate all configured servers |

**Validation Checks:**
- Service is running (checked via database)
- Connection can be established
- MCP protocol handshake succeeds
- Tools are properly registered
- Latency measurement

**Output Structure (Single Server):**
```json
{
  "results": [
    {
      "server": "filesystem",
      "valid": true,
      "health_status": "healthy",
      "validation_type": "mcp_validated",
      "tools_count": 5,
      "latency_ms": 15,
      "server_info": {
        "name": "filesystem",
        "version": "1.0.0",
        "protocol_version": "2024-11-05"
      },
      "issues": [],
      "message": "MCP validated with 5 tools"
    }
  ],
  "summary": {
    "total": 1,
    "valid": 1,
    "invalid": 0,
    "healthy": 1,
    "unhealthy": 0
  }
}
```

**Health Status Values:**
- `healthy` - Connected with <1s latency
- `slow` - Connected but >1s latency
- `auth_required` - Port responding but OAuth needed
- `unhealthy` - Connection failed or timeout
- `stopped` - Service not running
- `not_found` - Server not in configuration

**Validation Type Values:**
- `mcp_validated` - Full MCP handshake succeeded
- `auth_required` - OAuth authentication needed
- `not_running` - Service is not running
- `timeout` - Connection timed out
- `connection_error` - Failed to connect
- `config_error` - Server not in configuration

**Artifact Type:** `Card`

---

### mcp logs

View MCP server logs from database or disk files.

```bash
sp plugins mcp logs <server-name>
sp plugins mcp logs filesystem
sp plugins mcp logs filesystem --lines 100
sp plugins mcp logs filesystem --follow
sp plugins mcp logs filesystem --level error
sp plugins mcp logs filesystem --disk
sp plugins mcp logs --logs-dir /custom/path
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | No | MCP server name (shows all if not specified) |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--lines`, `-n` | `50` | Number of lines to show |
| `--follow`, `-f` | `false` | Follow log output continuously (disk only) |
| `--level` | All | Filter by log level: `debug`, `info`, `warn`, `error` |
| `--disk` | `false` | Force reading from disk files instead of database |
| `--logs-dir` | Profile path | Custom logs directory path |

**Log Level Filtering:**
- `debug` - Show all log levels
- `info` - Show INFO, WARN, ERROR (exclude DEBUG)
- `warn` - Show WARN and ERROR only
- `error` - Show ERROR only

**Output Structure:**
```json
{
  "service": "filesystem",
  "source": "database",
  "logs": [
    "2024-01-15 10:30:00 INFO [mcp-filesystem] Server started on port 9001",
    "2024-01-15 10:30:01 INFO [mcp-filesystem] Registered 5 tools"
  ],
  "log_files": []
}
```

**Artifact Type:** `Text`

---

### mcp list-packages

List MCP package names for build commands.

```bash
sp plugins mcp list-packages
sp --json mcp list-packages
sp plugins mcp list-packages --raw
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--raw` | Output as space-separated string with raw_packages field |

**Output Structure:**
```json
{
  "packages": [
    "mcp-filesystem",
    "mcp-database",
    "mcp-search"
  ]
}
```

**Output Structure (with --raw):**
```json
{
  "packages": [
    "mcp-filesystem",
    "mcp-database"
  ],
  "raw_packages": "mcp-filesystem mcp-database"
}
```

**Artifact Type:** `List` (or `CopyPasteText` with --raw)

---

## Complete MCP Management Flow Example

This flow demonstrates MCP server management:

```bash
# Phase 1: List configured servers
sp --json mcp list

# Phase 2: Check build status
sp --json mcp list-packages
sp build mcp --release

# Phase 3: Check running status
sp --json mcp status
sp --json mcp status --server filesystem

# Phase 4: Validate specific server with timeout
sp --json mcp validate filesystem --timeout 30

# Phase 5: Validate all servers
sp --json mcp validate --all

# Phase 6: Check logs with level filtering
sp plugins mcp logs filesystem --lines 20 --level error

# Phase 7: Follow logs in real-time
sp plugins mcp logs filesystem --follow
```

---

## MCP Server Configuration

MCP servers are configured in the services configuration:

```yaml
# services.yaml
mcp_servers:
  filesystem:
    enabled: true
    port: 9001
    command: "./target/debug/mcp-filesystem"
    binary: "mcp-filesystem"
    transport: stdio
    args: []
    env: {}
    oauth:
      required: false

  database:
    enabled: true
    port: 9002
    command: "./target/debug/mcp-database"
    binary: "mcp-database"
    transport: stdio
    args: []
    env:
      DATABASE_URL: "${DATABASE_URL}"
    oauth:
      required: true
```

---

## Troubleshooting MCP Servers

### Server Not Starting

```bash
# Check if binary exists
sp --json mcp status --server filesystem
# Look for "binary_exists": false or no debug/release binary

# Build the server
sp build mcp --server filesystem

# Re-check status
sp --json mcp status --server filesystem
```

### Connection Issues

```bash
# Validate connection with extended timeout
sp plugins mcp validate filesystem --timeout 30

# Check validation type in response
sp --json mcp validate filesystem | jq '.results[0].validation_type'

# Check logs for errors
sp plugins mcp logs filesystem --level error --lines 100
```

### Tool Registration Issues

```bash
# Validate and check tools count
sp --json mcp validate filesystem | jq '.results[0].tools_count'

# Check server info
sp --json mcp validate filesystem | jq '.results[0].server_info'
```

### Batch Validation

```bash
# Validate all servers at once
sp --json mcp validate --all

# Check summary
sp --json mcp validate --all | jq '.summary'

# Find unhealthy servers
sp --json mcp validate --all | jq '.results[] | select(.health_status != "healthy")'
```

---

## Error Handling

### Server Not Found

```bash
sp plugins mcp validate nonexistent
# Error: MCP server 'nonexistent' not found

sp plugins mcp logs nonexistent
# Error: Log file not found for service 'nonexistent'. Available: [...]
```

### Service Not Running

```bash
sp plugins mcp validate filesystem
# Returns: health_status: "stopped", validation_type: "not_running"
```

### Connection Timeout

```bash
sp plugins mcp validate filesystem --timeout 5
# Returns: health_status: "unhealthy", validation_type: "timeout"
```

### Non-Interactive Mode Requirements

```bash
sp plugins mcp validate
# Error: --service is required in non-interactive mode

# Solution: Use --all flag or provide service name
sp plugins mcp validate --all
sp plugins mcp validate filesystem
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json mcp list | jq .

# Extract specific fields
sp --json mcp list | jq '.servers[].name'
sp --json mcp status | jq '.servers[] | select(.running == true)'
sp --json mcp validate --all | jq '.results[] | select(.valid == false)'
sp --json mcp list-packages | jq '.packages[]'

# Check all server health
sp --json mcp status | jq '.summary'
sp --json mcp validate --all | jq '.summary'

# Get raw package list for shell scripts
sp --json mcp list-packages --raw | jq -r '.raw_packages'
```

---

## Integration with Services

MCP servers are started automatically with services:

```bash
# Start all services including MCP
sp infra services start

# Start only MCP servers
sp infra services start --mcp

# Stop MCP servers
sp infra services stop --mcp
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing servers
- [x] Interactive prompts have `--flag` equivalents
- [x] All documented flags are implemented
- [x] Log level filtering supported
- [x] Batch validation with `--all` flag
- [x] Configurable timeout for validation
- [x] Logs path from profile config (not hardcoded)
