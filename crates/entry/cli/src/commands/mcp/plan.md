# MCP Domain Implementation Plan

**Status: COMPLETED**

## Overview

Extracted MCP server management from `commands/agents/` into its own top-level domain at `commands/mcp/`.

## Changes Made

### New Files Created
```
commands/mcp/
├── mod.rs           # MCP routing and McpCommands enum
├── types.rs         # Output types (McpListOutput, McpValidateOutput, etc.)
├── list.rs          # List all MCP servers with status
├── list_packages.rs # List enabled package names for builds
└── validate.rs      # Validate MCP connection and list tools
```

### Files Modified
- `commands/mod.rs` - Added `pub mod mcp;`
- `lib.rs` - Added `Mcp` variant to `Commands` enum and routing

### Files Removed
- `commands/agents/mcp/` directory (moved to `commands/mcp/`)
- MCP references from `commands/agents/mod.rs`

## CLI Structure

**Before:**
```
sp agents mcp list
sp agents mcp validate <name>
sp agents mcp list-packages
```

**After:**
```
sp mcp list
sp mcp validate <name>
sp mcp list-packages
```

## Commands

| Command | Description |
|---------|-------------|
| `sp mcp list` | List all MCP servers with enabled/disabled status |
| `sp mcp validate <name>` | Validate connection and list available tools |
| `sp mcp list-packages` | Output enabled package names (for build scripts) |

## Verification

- [x] `cargo build -p systemprompt-cli` - Compiles without errors
- [x] `sp mcp --help` - Shows all subcommands
- [x] `sp agents --help` - Does NOT show mcp subcommand
