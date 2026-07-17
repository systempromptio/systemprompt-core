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


# Build CLI Commands

This document provides complete documentation for AI agents to use the build CLI commands. All commands support non-interactive mode for automation.

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
| `build core` | Build Rust workspace (systemprompt-core) | `Text` | No |
| `build mcp` | Build MCP extensions | `Text` | No |

---

## Core Commands

### build core

Build the Rust workspace (systemprompt-core).

```bash
sp build core
sp build core --release
sp build core --offline
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--release`, `-r` | `false` | Build in release mode (production) |
| `--offline` | `false` | Build with `SQLX_OFFLINE=true` (no database required) |

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

### build mcp

Build MCP (Model Context Protocol) extensions.

```bash
sp build mcp
sp build mcp --release
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--release` | `false` | Build in release mode |

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

# Phase 3: Verify builds
ls -la ./target/release/
```

---

## Development Build Flow

```bash
# Fast debug build for development
sp build core

# Build without a database available
sp build core --offline

# Build all MCP extensions
sp build mcp
```

---

## Production Build Flow

```bash
# Full production build
sp build core --release
sp build mcp --release

# Or use the services command for full startup
sp infra services start --skip-migrate
```

---

## Error Handling

### Build Errors

```bash
sp build core
# Error: Compilation failed. See errors above.

sp build mcp
# Error: Extension 'example' has no binary defined
```

### Missing Dependencies

```bash
sp build core
# Error: Missing toolchain. Install Rust via rustup.
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json build core | jq .

# Extract specific fields
sp --json build core | jq '.packages_built'
sp --json build mcp | jq '.servers_built[]'
```

---

## Integration with Services

The build commands integrate with the services workflow:

```bash
# Manual build then start
sp build core --release
sp build mcp --release
sp infra services start

# Or let services handle the build
sp infra services start
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


---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
