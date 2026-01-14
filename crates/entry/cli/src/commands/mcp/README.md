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
| `mcp list` | List MCP server configurations | `Table` | No |
| `mcp status` | Show running MCP server status | `Table` | No |
| `mcp validate <name>` | Validate MCP server connection | `Card` | Yes |
| `mcp logs <name>` | View MCP server logs | `Text` | No |
| `mcp list-packages` | List package names for build | `Table` | No |

---

## Core Commands

### mcp list

List all configured MCP servers from the services configuration.

```bash
sp mcp list
sp --json mcp list
sp mcp list --enabled
sp mcp list --disabled
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
      "command": "./target/debug/mcp-filesystem",
      "transport": "stdio",
      "tools_count": 5
    },
    {
      "name": "database",
      "port": 9002,
      "enabled": true,
      "command": "./target/debug/mcp-database",
      "transport": "stdio",
      "tools_count": 8
    }
  ],
  "total": 2
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `port`, `enabled`, `transport`, `tools_count`

---

### mcp status

Show running MCP server status with binary information.

```bash
sp mcp status
sp --json mcp status
sp mcp status --server filesystem
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--server` | All | Show status for specific server |

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
      "binary_path": "/var/www/html/systemprompt-core/target/debug/mcp-filesystem",
      "binary_exists": true,
      "uptime_seconds": 3600
    }
  ],
  "summary": {
    "total": 3,
    "running": 2,
    "stopped": 1
  }
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `enabled`, `running`, `pid`, `port`, `binary_exists`

---

### mcp validate

Validate MCP server connection and capabilities.

```bash
sp mcp validate <server-name>
sp --json mcp validate filesystem
sp mcp validate database --timeout 30
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | MCP server name to validate |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--timeout` | `10` | Connection timeout in seconds |

**Validation Checks:**
- Binary exists and is executable
- Server responds to initialization
- Tools are properly registered
- Connection is stable

**Output Structure:**
```json
{
  "server": "filesystem",
  "valid": true,
  "binary_check": "passed",
  "connection_check": "passed",
  "tools_check": "passed",
  "tools": [
    {
      "name": "read_file",
      "description": "Read contents of a file",
      "parameters": ["path"]
    },
    {
      "name": "write_file",
      "description": "Write contents to a file",
      "parameters": ["path", "content"]
    }
  ],
  "tools_count": 5,
  "latency_ms": 15,
  "message": "MCP server 'filesystem' validation passed"
}
```

**Artifact Type:** `Card`

---

### mcp logs

View MCP server logs.

```bash
sp mcp logs <server-name>
sp mcp logs filesystem
sp mcp logs filesystem --lines 100
sp mcp logs filesystem --follow
sp mcp logs filesystem --level error
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | MCP server name |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--lines`, `-n` | `50` | Number of lines to show |
| `--follow`, `-f` | `false` | Follow log output continuously |
| `--level` | All | Filter by log level: `debug`, `info`, `warn`, `error` |

**Output Structure:**
```json
{
  "server": "filesystem",
  "logs": [
    {
      "timestamp": "2024-01-15T10:30:00Z",
      "level": "INFO",
      "message": "MCP server started on port 9001"
    },
    {
      "timestamp": "2024-01-15T10:30:01Z",
      "level": "INFO",
      "message": "Registered 5 tools"
    }
  ],
  "total_lines": 50
}
```

**Artifact Type:** `Text`

---

### mcp list-packages

List MCP package names for build commands.

```bash
sp mcp list-packages
sp --json mcp list-packages
```

**Output Structure:**
```json
{
  "packages": [
    "mcp-filesystem",
    "mcp-database",
    "mcp-search"
  ],
  "total": 3
}
```

**Artifact Type:** `Table`

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

# Phase 4: Validate specific server
sp --json mcp validate filesystem

# Phase 5: Check logs
sp mcp logs filesystem --lines 20

# Phase 6: Validate all servers
for server in $(sp --json mcp list | jq -r '.servers[].name'); do
  echo "Validating $server..."
  sp --json mcp validate "$server"
done
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
    transport: stdio
    args: []
    env: {}

  database:
    enabled: true
    port: 9002
    command: "./target/debug/mcp-database"
    transport: stdio
    args: []
    env:
      DATABASE_URL: "${DATABASE_URL}"
```

---

## Troubleshooting MCP Servers

### Server Not Starting

```bash
# Check if binary exists
sp --json mcp status --server filesystem
# Look for "binary_exists": false

# Build the server
sp build mcp --server filesystem

# Re-check status
sp --json mcp status --server filesystem
```

### Connection Issues

```bash
# Validate connection
sp mcp validate filesystem --timeout 30

# Check logs for errors
sp mcp logs filesystem --level error --lines 100
```

### Tool Registration Issues

```bash
# Validate and see tools
sp --json mcp validate filesystem | jq '.tools'
```

---

## Error Handling

### Server Not Found

```bash
sp mcp validate nonexistent
# Error: MCP server 'nonexistent' not found in configuration

sp mcp logs nonexistent
# Error: MCP server 'nonexistent' not found
```

### Binary Not Found

```bash
sp mcp validate filesystem
# Error: Binary not found at /path/to/mcp-filesystem. Run 'build mcp' first.
```

### Connection Errors

```bash
sp mcp validate filesystem
# Error: Failed to connect to MCP server 'filesystem'. Server may not be running.
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
sp --json mcp validate filesystem | jq '.tools[].name'
sp --json mcp list-packages | jq '.packages[]'

# Check all server health
sp --json mcp status | jq '.summary'
```

---

## Integration with Services

MCP servers are started automatically with services:

```bash
# Start all services including MCP
sp services start

# Start only MCP servers
sp services start --mcp

# Stop MCP servers
sp services stop --mcp
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
