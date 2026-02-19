# MCP Server Type: Internal vs External

## What Changed in `systemprompt-claude-marketplace`

We added an explicit `type: internal | external` field to MCP server configuration. This distinguishes servers that run as local binaries (internal) from remote endpoints (external).

### YAML Config (`services/mcp/*.yaml`)

```yaml
# Before
mcp_servers:
  excalidraw:
    binary: ""
    port: 0
    endpoint: https://mcp.excalidraw.com/mcp

# After
mcp_servers:
  excalidraw:
    type: external          # <-- NEW FIELD
    binary: ""
    port: 0
    endpoint: https://mcp.excalidraw.com/mcp
```

Values: `internal` (has binary, managed locally) or `external` (remote endpoint only).

### Extension Rust Types (`extensions/web/src/admin/types/plugins.rs`)

```rust
pub struct McpServerDetail {
    pub server_type: String,   // <-- ADDED
    // ... rest unchanged
}

pub struct CreateMcpRequest {
    pub server_type: String,   // <-- ADDED, defaults to "internal"
    // ...
}

pub struct UpdateMcpRequest {
    pub server_type: Option<String>,  // <-- ADDED
    // ...
}
```

### Extension YAML Parsing (`extensions/web/src/admin/repositories/mcp_servers.rs`)

Reads `type` from YAML with fallback heuristic: if `binary` is empty, assumes `external`.

### SSR Handler (`extensions/web/src/admin/handlers/ssr/ssr_mcp.rs`)

- Queries `services` table for status of internal servers
- Queries `mcp_tool_executions` for usage stats per server
- Passes `is_internal`, `is_external`, `service_status`, `total_executions`, `success_rate` to template

### Admin UI (`storage/files/admin/templates/mcp-servers.hbs`)

- New "Type" column with Internal/External badge
- "Connection" column replaces old Binary + Port columns
  - External: shows endpoint URL
  - Internal: shows service status badge + usage stats

---

## What Needs to Change in Core

### 1. `Deployment` struct — Add `server_type` field

**File:** `crates/shared/models/src/mcp/deployment.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum McpServerType {
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "external")]
    External,
}

impl Default for McpServerType {
    fn default() -> Self {
        Self::Internal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    #[serde(default)]
    pub server_type: McpServerType,    // <-- ADD THIS (aliased from "type" in YAML)
    pub binary: String,
    pub package: Option<String>,
    pub port: u16,
    pub endpoint: String,
    pub enabled: bool,
    pub display_in_web: bool,
    // ... rest unchanged
}
```

**Note:** The YAML field is `type` but Rust reserves that keyword. Use `#[serde(rename = "type")]` or `#[serde(alias = "type")]` on the field:

```rust
#[serde(default, alias = "type")]
pub server_type: McpServerType,
```

### 2. `McpServerConfig` struct — Add `server_type` field

**File:** `crates/shared/models/src/mcp/server.rs`

```rust
pub struct McpServerConfig {
    pub server_type: McpServerType,    // <-- ADD THIS
    pub name: String,
    pub binary: String,
    // ... rest unchanged
}
```

This must be populated when building `McpServerConfig` from `Deployment` (wherever that conversion happens in the config loader / registry builder).

### 3. `mod.rs` exports — Re-export the enum

**File:** `crates/shared/models/src/mcp/mod.rs`

```rust
pub use deployment::{Deployment, DeploymentConfig, McpServerType, OAuthRequirement, Settings};
```

### 4. Validator — Validate type-specific constraints

**File:** `crates/domain/mcp/src/services/registry/validator.rs`

Add a new validation function:

```rust
fn validate_server_types(config: &RegistryConfig) -> Result<()> {
    let issues: Vec<String> = config
        .servers
        .iter()
        .filter(|s| s.enabled)
        .filter_map(|s| {
            match s.server_type {
                McpServerType::Internal => {
                    // Internal servers MUST have a binary
                    if s.binary.is_empty() {
                        return Some(format!("{}: internal server has no binary", s.name));
                    }
                    // Internal servers MUST have a valid port
                    if s.port == 0 {
                        return Some(format!("{}: internal server has no port", s.name));
                    }
                    None
                }
                McpServerType::External => {
                    // External servers MUST have an endpoint
                    if s.endpoint().is_empty() || s.endpoint().starts_with("http://localhost") {
                        return Some(format!("{}: external server needs a remote endpoint", s.name));
                    }
                    // External servers should NOT have a binary
                    if !s.binary.is_empty() {
                        return Some(format!("{}: external server should not have a binary", s.name));
                    }
                    None
                }
            }
        })
        .collect();

    if issues.is_empty() {
        return Ok(());
    }
    Err(anyhow::anyhow!("Server type issues:\n{}", issues.join("\n")))
}
```

Call it from `validate_registry()`:

```rust
pub fn validate_registry(config: &RegistryConfig) -> Result<()> {
    validate_port_conflicts(config)?;
    validate_server_configs(config)?;
    validate_oauth_configs(config)?;
    validate_server_types(config)?;    // <-- ADD
    Ok(())
}
```

### 5. `ServicesConfig` validation — Skip port checks for external servers

**File:** `crates/shared/models/src/services/mod.rs`

In `validate_port_conflicts()` and `validate_mcp_port_ranges()`, external servers should be excluded since they don't bind local ports:

```rust
// Only check port conflicts for internal servers
.filter(|s| s.server_type == McpServerType::Internal)
```

### 6. Orchestrator — Skip start/stop for external servers

**File:** `crates/domain/mcp/src/services/orchestrator/mod.rs`

When starting/stopping/building services, external servers must be skipped (they have no binary to spawn). They should only be health-checked via their endpoint.

```rust
// In start_services, filter to internal only:
let internal_servers: Vec<_> = servers
    .iter()
    .filter(|s| s.server_type == McpServerType::Internal)
    .collect();
```

External servers should still be included in `list_services()` and health checks, but the health check should hit the remote endpoint rather than check for a local PID.

### 7. Monitoring — Handle external health checks

**File:** `crates/domain/mcp/src/services/monitoring/status.rs`

`get_service_status()` currently assumes all servers are local processes. For external servers:
- Skip PID checks
- Health check should be an HTTP request to the endpoint
- State is derived purely from endpoint reachability (not process status)

### 8. Database sync — External servers in `services` table

**File:** `crates/domain/mcp/src/services/database/mod.rs`

External servers should still be registered in the `services` table for status tracking, but with `pid = NULL` and `binary_mtime = NULL`. Consider adding a `server_type` column to the `services` table:

```sql
ALTER TABLE services ADD COLUMN server_type TEXT NOT NULL DEFAULT 'internal';
```

---

## Summary of Core Files to Modify

| File | Change |
|------|--------|
| `crates/shared/models/src/mcp/deployment.rs` | Add `McpServerType` enum + field on `Deployment` |
| `crates/shared/models/src/mcp/server.rs` | Add `server_type` field to `McpServerConfig` |
| `crates/shared/models/src/mcp/mod.rs` | Re-export `McpServerType` |
| `crates/shared/models/src/services/mod.rs` | Skip port validation for external servers |
| `crates/domain/mcp/src/services/registry/validator.rs` | Add type-specific validation rules |
| `crates/domain/mcp/src/services/orchestrator/mod.rs` | Skip start/stop/build for external |
| `crates/domain/mcp/src/services/monitoring/status.rs` | Handle external health checks differently |
| `crates/domain/mcp/src/services/database/mod.rs` | Handle external server registration |
| `crates/domain/agent/schema/services.sql` | Add `server_type` column |
| Config loader (enhanced + basic) | Propagate `server_type` from Deployment to McpServerConfig |

## Existing YAML Files to Update

All files in `services/mcp/` need the `type` field added. Current state in marketplace repo:

| File | Type |
|------|------|
| `services/mcp/excalidraw.yaml` | `type: external` (done) |
| `services/mcp/systemprompt.yaml` | `type: internal` (done) |
