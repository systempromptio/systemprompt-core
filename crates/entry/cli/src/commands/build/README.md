# Build CLI Commands

This document provides complete documentation for AI agents to use the build CLI commands. All commands support non-interactive mode for automation.

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
| `build core` | Build Rust workspace (systemprompt-core) | `Text` | No |
| `build web` | Build web frontend | `Text` | No |
| `build mcp` | Build MCP extensions | `Text` | No |

---

## Core Commands

### build core

Build the Rust workspace (systemprompt-core).

```bash
sp build core
sp build core --release
sp build core --package systemprompt-cli
sp build core --features "feature1,feature2"
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--release` | `false` | Build in release mode |
| `--package` | All | Build specific package only |
| `--features` | None | Comma-separated features to enable |
| `--all-features` | `false` | Enable all features |
| `--no-default-features` | `false` | Disable default features |

**Output Structure:**
```json
{
  "success": true,
  "mode": "debug",
  "packages_built": ["systemprompt-cli", "systemprompt-core"],
  "duration_seconds": 45,
  "output_path": "/var/www/html/systemprompt-core/target/debug",
  "message": "Build completed successfully"
}
```

**Artifact Type:** `Text`

---

### build web

Build the web frontend.

```bash
sp build web
sp build web --production
sp build web --watch
sp build web --minify
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--production` | `false` | Build for production (optimized) |
| `--watch` | `false` | Watch for changes and rebuild |
| `--minify` | `false` | Minify output (CSS/JS) |
| `--no-sourcemaps` | `false` | Disable source maps |

**Output Structure:**
```json
{
  "success": true,
  "mode": "development",
  "output_path": "/var/www/html/tyingshoelaces/services/web/dist",
  "files_generated": 15,
  "total_size_bytes": 524288,
  "duration_seconds": 12,
  "message": "Web build completed successfully"
}
```

**Artifact Type:** `Text`

---

### build mcp

Build MCP (Model Context Protocol) extensions.

```bash
sp build mcp
sp build mcp --release
sp build mcp --server filesystem
sp build mcp --all
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--release` | `false` | Build in release mode |
| `--server` | None | Build specific MCP server only |
| `--all` | `false` | Build all MCP servers |

**Output Structure:**
```json
{
  "success": true,
  "mode": "debug",
  "servers_built": ["filesystem", "database", "search"],
  "duration_seconds": 30,
  "output_path": "/var/www/html/systemprompt-core/target/debug",
  "message": "MCP build completed successfully"
}
```

**Artifact Type:** `Text`

---

## Complete Build Flow Example

This flow demonstrates building all components:

```bash
# Phase 1: Build core Rust workspace
sp build core --release

# Phase 2: Build MCP extensions
sp build mcp --release

# Phase 3: Build web frontend
sp build web --production --minify

# Phase 4: Verify builds
ls -la ./target/release/
ls -la ./services/web/dist/
```

---

## Development Build Flow

```bash
# Fast debug build for development
sp build core --package systemprompt-cli

# Watch mode for web development
sp build web --watch

# Build specific MCP server
sp build mcp --server filesystem
```

---

## Production Build Flow

```bash
# Full production build
sp build core --release
sp build mcp --release
sp build web --production --minify --no-sourcemaps

# Or use the services command for full startup
sp services start --skip-migrate
```

---

## Error Handling

### Build Errors

```bash
sp build core
# Error: Compilation failed. See errors above.

sp build web
# Error: Web build failed. Node.js/npm not found.

sp build mcp --server nonexistent
# Error: MCP server 'nonexistent' not found in configuration
```

### Missing Dependencies

```bash
sp build core
# Error: Missing toolchain. Install Rust via rustup.

sp build web
# Error: Missing npm dependencies. Run 'npm install' first.
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json build core | jq .

# Extract specific fields
sp --json build core | jq '.packages_built'
sp --json build web | jq '.total_size_bytes'
sp --json build mcp | jq '.servers_built[]'
```

---

## Integration with Services

The build commands integrate with the services workflow:

```bash
# Manual build then start
sp build core --release
sp build mcp --release
sp build web --production
sp services start --skip-web

# Or let services handle the build
sp services start
# This automatically builds as needed
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
