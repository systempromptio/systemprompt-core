# Services Domain Plan

## Purpose
The `services` domain manages running processes: agents, MCP servers, and API server lifecycle.

## CLI Structure
```
systemprompt services start [--all|--api|--agents|--mcp] [--skip-web] [--skip-migrate]
systemprompt services stop [--all|--api|--agents|--mcp] [--force]
systemprompt services restart [api|agent <id>|mcp <name>] [--failed]
systemprompt services status [--detailed] [--json] [--health]
systemprompt services cleanup
systemprompt services serve [--foreground] [--kill-port-process]
```

## Files
```
commands/services/
├── mod.rs       # ServicesCommands enum and execute routing
├── start.rs     # Start services
├── stop.rs      # Stop services
├── restart.rs   # Restart services
├── status.rs    # Service status display
├── serve.rs     # API server startup
└── cleanup.rs   # Cleanup orphaned processes
```

## Commands

### start
Start API, agents, and/or MCP servers.

**Flags:**
- `--all` - Start all services
- `--api` - Start API server only
- `--agents` - Start agents only
- `--mcp` - Start MCP servers only
- `--foreground` - Run in foreground (default)
- `--skip-web` - Skip web asset build
- `--skip-migrate` - Skip database migrations

### stop
Stop running services gracefully.

**Flags:**
- `--all` - Stop all services
- `--api` - Stop API server only
- `--agents` - Stop agents only
- `--mcp` - Stop MCP servers only
- `--force` - Force stop (SIGKILL)

### restart
Restart services.

**Subcommands:**
- `api` - Restart API server
- `agent <agent_id>` - Restart specific agent
- `mcp <server_name>` - Restart specific MCP server

**Flags:**
- `--failed` - Restart only failed services

### status
Show detailed service status.

**Flags:**
- `--detailed` - Show detailed information
- `--json` - Output as JSON
- `--health` - Include health check results

**Output columns:**
- Name
- Type (API/Agent/MCP)
- Status (Running/Stopped/Failed)
- PID
- Port
- Action (what needs to happen)

### cleanup
Clean up orphaned processes and stale entries.

### serve
Start API server (automatically starts agents and MCP servers).

**Flags:**
- `--foreground` - Run in foreground mode
- `--kill-port-process` - Kill process using the port if occupied

## Changes Required

1. Remove `db` submodule import
2. Remove `scheduler` submodule import
3. Remove `Db` and `Scheduler` variants from `ServicesCommands`
4. Remove corresponding match arms in `execute` function

## Dependencies
- `systemprompt_runtime::AppContext`
- `systemprompt_core_scheduler::{ServiceStateManager, RuntimeStatus, VerifiedServiceState}`
- `systemprompt_loader::ConfigLoader`
