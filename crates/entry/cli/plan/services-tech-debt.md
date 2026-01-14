# Services CLI - Tech Debt & Enhancement Plan

## Test Results Summary

All services commands are **fully functional**:

| Command | Status | Notes |
|---------|--------|-------|
| `services status` | ✅ Working | Supports `--detailed`, `--health`, `--json` |
| `services start` | ✅ Working | Supports `--all`, `--api`, `--agents`, `--mcp`, `--skip-web`, `--skip-migrate` |
| `services stop` | ✅ Working | Supports `--all`, `--api`, `--agents`, `--mcp`, `--force` |
| `services restart` | ✅ Working | Supports `api`, `agent <name>`, `mcp <name>`, `--failed` |
| `services cleanup` | ✅ Working | Cleans orphaned processes and stale entries |
| `services serve` | ✅ Working | Supports `--foreground`, `--kill-port-process` |

---

## README Compliance

### Forbidden Pattern Checks: ✅ ALL PASS

```
✅ No println! usage
✅ No unwrap() usage (excluding unwrap_or variants)
✅ No expect() usage
✅ No env::set_var("SYSTEMPROMPT_NON_INTERACTIVE"...) manipulation
```

### Required Pattern Compliance

| Requirement | Status |
|-------------|--------|
| All `execute` functions accept `config: &CliConfig` | ✅ Compliant |
| All commands return `CommandResult<T>` or handle output via `CliService` | ✅ Compliant |
| All output types derive `Serialize`, `Deserialize`, `JsonSchema` | ✅ Compliant |
| JSON output via `--json` flag | ✅ Compliant |
| Port conflict handling with `--kill-port-process` flag | ✅ Compliant |

---

## Tech Debt

### 1. Missing `--yes` Flag on `services cleanup` (Medium Priority)

**Location:** `crates/entry/cli/src/commands/services/mod.rs:83-84`

**Issue:** Per README section 1.5, destructive operations should have a `--yes`/`-y` flag. The cleanup command stops running services without confirmation.

**Current:**
```rust
#[command(about = "Clean up orphaned processes and stale entries")]
Cleanup,
```

**Should be:**
```rust
#[command(about = "Clean up orphaned processes and stale entries")]
Cleanup {
    #[arg(short = 'y', long, help = "Skip confirmation")]
    yes: bool,
    #[arg(long, help = "Preview cleanup without executing")]
    dry_run: bool,
},
```

### 2. API Restart Not Implemented (Low Priority)

**Location:** `crates/entry/cli/src/commands/services/restart.rs:16-23`

**Issue:** The `services restart api` command just shows instructions instead of actually restarting.

**Current behavior:**
```
Restarting API Server
⚠ API server restart via CLI is not currently supported
ℹ To restart the API server:
ℹ   1. Stop the current server (Ctrl+C if running in foreground)
ℹ   2. Run: just api
```

**Recommendation:** Either implement proper API restart or remove the subcommand and provide clear error message.

### 3. Unused `_config` Parameters (Low Priority)

**Locations:**
- `stop.rs:10` - `_config: &CliConfig` unused
- `restart.rs:16` - `_config: &CliConfig` unused
- `restart.rs:28` - `_config: &CliConfig` unused
- `restart.rs:50` - `_config: &CliConfig` unused
- `restart.rs:68` - `_config: &CliConfig` unused
- `cleanup.rs:8` - `_config: &CliConfig` unused

**Issue:** These functions accept config but don't use it for interactive/non-interactive mode detection.

**Impact:** Low - signatures are correct per README, but the config isn't being used for conditional behavior.

### 4. Hardcoded Port 8080 (Low Priority)

**Location:** `crates/entry/cli/src/commands/services/serve.rs:18`

**Issue:** API port is hardcoded to 8080.

```rust
let port = 8080u16;
```

**Recommendation:** Read from profile configuration.

---

## Friction Points

### 1. Agent/MCP Start Messaging is Confusing

When running `services start --agents` or `services start --mcp`, the command outputs:
```
ℹ Agents start automatically with the API server
ℹ MCP servers start automatically with the API server
```

This is technically correct but confusing. Users expect `--agents` to start just agents.

**Recommendation:** Either:
1. Actually implement standalone agent/MCP starting, or
2. Remove the `--agents` and `--mcp` flags if they're not supported standalone

### 2. No Service Dependencies Documentation

The README documents startup order but doesn't clearly explain that agents and MCP servers are automatically managed by the API server lifecycle.

---

## Enhancement Suggestions

### 1. Add `--watch` Flag to Status (High Value)

```bash
sp services status --watch
sp services status --watch --interval 5
```

Auto-refresh status display every N seconds.

### 2. Add Health Check Details (Medium Value)

When `--health` flag is used, show actual health check results:

```json
{
  "health": {
    "status": "degraded",
    "checks": [
      {"name": "memory", "status": "ok", "value": "45%"},
      {"name": "cpu", "status": "warning", "value": "85%"},
      {"name": "database", "status": "ok", "latency_ms": 12}
    ]
  }
}
```

### 3. Add `--timeout` Flag for Operations (Medium Value)

```bash
sp services stop --timeout 30
sp services cleanup --timeout 60
```

Allow configuring how long to wait for graceful shutdown.

### 4. Add `services logs` Subcommand (High Value)

Convenience alias to stream service-specific logs:

```bash
sp services logs api --follow
sp services logs agent content --lines 100
sp services logs mcp filesystem
```

### 5. Add Resource Usage to Status (Medium Value)

Include CPU/memory usage in status output:

```json
{
  "name": "content",
  "status": "running",
  "pid": 12345,
  "cpu_percent": 2.5,
  "memory_mb": 128
}
```

### 6. Implement Proper Daemon Mode (Low Priority)

Currently `--foreground` is ignored and daemon mode isn't supported:
```
⚠ Daemon mode not supported, running in foreground
```

Consider implementing proper daemonization or removing the flag.

---

## Implementation Priority

| Item | Priority | Effort | Impact |
|------|----------|--------|--------|
| Add `--yes` to cleanup | High | Low | Compliance |
| Add `--watch` to status | Medium | Medium | UX |
| Fix agent/MCP start messaging | Medium | Low | UX/Clarity |
| Add health check details | Medium | Medium | Observability |
| Add `--timeout` flags | Low | Low | Flexibility |
| Implement API restart | Low | High | Feature completeness |
| Add resource usage to status | Low | Medium | Observability |
