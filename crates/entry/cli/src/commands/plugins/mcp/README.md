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


# MCP CLI Commands

Every MCP tool call goes through one audited path you own. These commands list, inspect, validate, and invoke the Model Context Protocol servers configured on your instance. All of them run non-interactively for automation.

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
| `plugins mcp list` | List configured MCP servers | `Table` | No |
| `plugins mcp status` | Show MCP server runtime status | `Table` | Yes |
| `plugins mcp validate [name]` | Validate MCP server configurations | `Card` | Yes |
| `plugins mcp validate --all` | Validate all configured servers | `Card` | Yes |
| `plugins mcp logs [name]` | Tail logs for an MCP server | `Text` | No |
| `plugins mcp list-packages` | List discovered MCP packages from the registry | `List` | Yes |
| `plugins mcp tools` | List tools exposed by enabled MCP servers | `Table` | Yes |
| `plugins mcp call <server> <tool>` | Invoke a tool on an MCP server | `Card` | Yes |

---

## Core Commands

### mcp list

List configured MCP servers from the services configuration, filterable by enabled or disabled state.

```bash
sp plugins mcp list
sp --json plugins mcp list
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
      "display_name": "filesystem",
      "server_type": "internal",
      "port": 9001,
      "enabled": true,
      "status": "ready",
      "binary_debug": "/path/to/target/debug/mcp-filesystem",
      "binary_release": "/path/to/target/release/mcp-filesystem"
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
- `remote` - External server, enabled
- `disabled` - External server, disabled

**Artifact Type:** `Table`
**Columns:** `name`, `server_type`, `port`, `enabled`, `status`, `endpoint`, `binary_debug`, `binary_release`

---

### mcp status

Report health and running state of configured MCP servers via the orchestrator.

```bash
sp plugins mcp status
sp --json plugins mcp status
sp plugins mcp status --detailed
sp plugins mcp status --server content-manager
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--detailed`, `-d` | `false` | Show detailed output including binary paths |
| `--server` | All | Filter to a specific server by name |

**Output Structure:**
```json
{
  "servers": [
    {
      "name": "filesystem",
      "server_type": "internal",
      "port": 9001,
      "enabled": true,
      "running": true,
      "health": "healthy",
      "pid": 12345,
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
**Columns:** `name`, `server_type`, `port`, `enabled`, `running`, `health`, `pid`, `endpoint`, `release_binary`, `debug_binary`

---

### mcp validate

Validate connectivity to one or all configured MCP servers with an authenticated handshake. Returns tools count, latency, and server info.

```bash
sp plugins mcp validate <server-name>
sp --json plugins mcp validate filesystem
sp plugins mcp validate database --timeout 30
sp plugins mcp validate --all
sp plugins mcp validate --all --timeout 5
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<server>` | Yes* | MCP server name (*not required with `--all`, or in non-interactive mode where all servers are validated) |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--service <name>` | None | Alias for the positional MCP server name (conflicts with the positional) |
| `--all` | `false` | Validate all configured servers |
| `--timeout` | `10` | Connection timeout in seconds |

**Validation Checks:**
- Service is running (checked via database)
- Connection can be established
- MCP protocol handshake succeeds
- Tools are registered
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
- `healthy` - Connected and responsive
- `auth_required` - Port responding but OAuth needed
- `unhealthy` - Connection failed or timed out
- `stopped` - Service not running
- `not_found` - Server not in configuration

**Validation Type Values:**
- `mcp_validated` - Full MCP handshake succeeded
- `not_running` - Service is not running
- `timeout` - Connection timed out
- `connection_error` - Failed to connect
- `config_error` - Server not in configuration
- `database_error` - Failed to read service status

**Artifact Type:** `Card`

---

### mcp logs

Tail logs for an MCP server, read from the database by default and falling back to disk files.

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
| `<server>` | No | MCP server name (shows all MCP logs if not specified) |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--lines`, `-n`, `--tail` | `50` | Number of lines to show |
| `--follow`, `-f` | `false` | Follow log output continuously (disk only) |
| `--disk` | `false` | Force reading from disk files instead of database |
| `--logs-dir` | Profile path | Custom logs directory path |
| `--level` | All | Filter by log level: `debug`, `info`, `warn`, `error` |

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

List discovered MCP package names from the registry, for use in build commands.

```bash
sp plugins mcp list-packages
sp --json plugins mcp list-packages
sp plugins mcp list-packages --raw
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--raw` | Output as a space-separated string in the `raw_packages` field |

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

**Artifact Type:** `List` (or `CopyPasteText` with `--raw`)

---

### mcp tools

List tools advertised by running MCP servers, optionally with full schemas.

```bash
sp plugins mcp tools
sp --json plugins mcp tools
sp plugins mcp tools --server filesystem
sp plugins mcp tools --detailed
sp plugins mcp tools --schema
sp plugins mcp tools --timeout 60
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--server`, `-s` | All running | Filter to a specific MCP server |
| `--detailed` | `false` | Show full input/output schemas in JSON output |
| `--schema` | `false` | Display parameter schemas in a readable format |
| `--timeout` | `30` | Timeout in seconds |

**Output Structure:**
```json
{
  "tools": [
    {
      "name": "read_file",
      "server": "filesystem",
      "description": "Read the contents of a file",
      "parameters_count": 1
    }
  ],
  "summary": {
    "total_tools": 5,
    "servers_queried": 1
  }
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `server`, `description`, `parameters_count`

---

### mcp call

Invoke a named tool on a running MCP server with JSON arguments and render the result.

```bash
sp plugins mcp call <server> <tool>
sp plugins mcp call systemprompt systemprompt --args '{"command":"core skills list"}'
sp plugins mcp call filesystem read_file -a '{"path":"/etc/hosts"}'
sp plugins mcp call database query --args '{"sql":"SELECT 1"}' --timeout 60
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<server>` | Yes (non-interactive) | MCP server name |
| `<tool>` | Yes (non-interactive) | Tool name to execute |

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--args`, `-a` | None | Tool arguments as a JSON string |
| `--timeout` | `30` | Timeout in seconds |

**Output Structure:**
```json
{
  "server": "filesystem",
  "tool": "read_file",
  "success": true,
  "content": [
    {
      "kind": "text",
      "text": "file contents here"
    }
  ],
  "execution_time_ms": 42
}
```

**Artifact Type:** `Card`

---

## Complete MCP Management Flow Example

This flow demonstrates MCP server management:

```bash
# Phase 1: List configured servers
sp --json plugins mcp list

# Phase 2: Check build status
sp --json plugins mcp list-packages
sp build mcp --release

# Phase 3: Check running status
sp --json plugins mcp status
sp --json plugins mcp status --server filesystem

# Phase 4: Validate a specific server with timeout
sp --json plugins mcp validate filesystem --timeout 30

# Phase 5: Validate all servers
sp --json plugins mcp validate --all

# Phase 6: Inspect the tools a server exposes
sp --json plugins mcp tools --server filesystem

# Phase 7: Invoke a tool
sp --json plugins mcp call filesystem read_file -a '{"path":"/etc/hosts"}'

# Phase 8: Check logs with level filtering
sp plugins mcp logs filesystem --lines 20 --level error

# Phase 9: Follow logs in real-time
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
# Check whether the binary exists
sp --json plugins mcp status --server filesystem
# Look for a null debug/release binary or a not-built status

# Build the server
sp build mcp --server filesystem

# Re-check status
sp --json plugins mcp status --server filesystem
```

### Connection Issues

```bash
# Validate connection with an extended timeout
sp plugins mcp validate filesystem --timeout 30

# Check the validation type in the response
sp --json plugins mcp validate filesystem | jq '.results[0].validation_type'

# Check logs for errors
sp plugins mcp logs filesystem --level error --lines 100
```

### Tool Registration Issues

```bash
# Validate and check the tools count
sp --json plugins mcp validate filesystem | jq '.results[0].tools_count'

# List the tools directly
sp --json plugins mcp tools --server filesystem | jq '.tools[].name'
```

### Batch Validation

```bash
# Validate all servers at once
sp --json plugins mcp validate --all

# Check the summary
sp --json plugins mcp validate --all | jq '.summary'

# Find unhealthy servers
sp --json plugins mcp validate --all | jq '.results[] | select(.health_status != "healthy")'
```

---

## Error Handling

### Server Not Found

```bash
sp plugins mcp validate nonexistent
# Error: MCP server 'nonexistent' not found

sp plugins mcp call nonexistent sometool
# Error: MCP server 'nonexistent' not found in configuration
```

### Service Not Running

```bash
sp plugins mcp validate filesystem
# Returns: health_status: "stopped", validation_type: "not_running"

sp plugins mcp call filesystem read_file
# Error: MCP server 'filesystem' is not running
```

### Connection Timeout

```bash
sp plugins mcp validate filesystem --timeout 5
# Returns: health_status: "unhealthy", validation_type: "timeout"
```

### Non-Interactive Mode

```bash
# validate with no server name validates all configured servers
sp --non-interactive plugins mcp validate

# call requires an explicit server and tool
sp --non-interactive plugins mcp call filesystem read_file -a '{"path":"/etc/hosts"}'
```

---

## JSON Output

All commands support the `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json plugins mcp list | jq .

# Extract specific fields
sp --json plugins mcp list | jq '.servers[].name'
sp --json plugins mcp status | jq '.servers[] | select(.running == true)'
sp --json plugins mcp validate --all | jq '.results[] | select(.valid == false)'
sp --json plugins mcp list-packages | jq '.packages[]'
sp --json plugins mcp tools | jq '.tools[] | {name, server}'

# Check summaries
sp --json plugins mcp status | jq '.summary'
sp --json plugins mcp validate --all | jq '.summary'

# Get the raw package list for shell scripts
sp --json plugins mcp list-packages --raw | jq -r '.raw_packages'
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

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
