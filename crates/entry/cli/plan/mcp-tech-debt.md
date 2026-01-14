# MCP Command Tech Debt & Enhancement Report

**Generated:** 2026-01-14
**Audit Scope:** `/crates/entry/cli/src/commands/mcp/`
**Status:** All commands functional with documentation discrepancies

---

## Executive Summary

The MCP command module is **largely compliant** with CLI standards. All forbidden patterns are avoided, CommandResult types are properly used, and dual-mode operation is supported. However, there are **documentation-implementation gaps** and **missing features** documented in the README.

### Compliance Score: 8/10

---

## 1. Documentation vs Implementation Gaps

### 1.1 `mcp list` - Missing `--disabled` Flag

**README Documents:**
```bash
sp mcp list --disabled
```

**Implementation:** Only `--enabled` flag exists in `ListArgs`.

**File:** `list.rs:14-17`
```rust
#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled servers")]
    pub enabled: bool,
}
```

**Resolution:** Either add `--disabled` flag or update README to remove it.

---

### 1.2 `mcp status` - Wrong Flag Documentation

**README Documents:**
```bash
sp mcp status --server filesystem
```
States flag is `--server` to show status for specific server.

**Implementation:** Has `--detailed` flag instead (`-d, --detailed`).

**File:** `status.rs:14-17`
```rust
#[derive(Debug, Clone, Copy, Args)]
pub struct StatusArgs {
    #[arg(long, short, help = "Show detailed output including binary paths")]
    pub detailed: bool,
}
```

**Resolution:** Update README to document `--detailed` instead of `--server`, or add server filtering capability.

---

### 1.3 `mcp validate` - Missing `--timeout` Flag

**README Documents:**
```bash
sp mcp validate database --timeout 30
```

**Implementation:** No `--timeout` flag exists.

**File:** `validate.rs:16-21`
```rust
#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "MCP server name (required in non-interactive mode)")]
    pub service: Option<String>,
}
```

**Resolution:** Either add `--timeout` flag or update README.

---

### 1.4 `mcp logs` - Missing `--level` Flag

**README Documents:**
```bash
sp mcp logs filesystem --level error
```

**Implementation:** No `--level` flag exists.

**File:** `logs.rs:19-40`
```rust
pub struct LogsArgs {
    pub service: Option<String>,
    pub lines: usize,
    pub follow: bool,
    pub disk: bool,
    pub logs_dir: Option<String>,
}
```

**Resolution:** Either add `--level` flag for log filtering or update README.

---

### 1.5 `mcp validate` - Output Structure Mismatch

**README Documents:**
```json
{
  "tools": [
    {
      "name": "read_file",
      "description": "Read contents of a file",
      "parameters": ["path"]
    }
  ],
  "latency_ms": 15
}
```

**Implementation:** Output lacks tools list, latency, and detailed validation checks.

**File:** `types.rs:25-31`
```rust
pub struct McpValidateOutput {
    pub server: String,
    pub valid: bool,
    pub tools_count: usize,  // Always 0 in current impl
    pub issues: Vec<String>,
}
```

**Resolution:** Enhance validation to capture actual tools from MCP server.

---

## 2. Compliance Checklist

| Requirement | Status | Notes |
|-------------|--------|-------|
| All `execute` functions accept `config: &CliConfig` | PASS | All commands have config |
| All commands return `CommandResult<T>` | PASS | Proper artifact types used |
| All output types derive required traits | PASS | `Serialize`, `Deserialize`, `JsonSchema` |
| No `println!` / `eprintln!` | PASS | Clean |
| No `unwrap()` / `expect()` | PASS | Uses `?` with `.context()` |
| JSON output supported via `--json` flag | PASS | Works correctly |
| Proper error messages for missing servers | PASS | Clear error messages |
| Interactive prompts have flag equivalents | PASS | `resolve_input` pattern used |

---

## 3. Code Quality Issues

### 3.1 Unused `_config` Parameter

Several functions accept `config: &CliConfig` but don't use it (prefix with `_`).

**Files:**
- `list.rs:19` - `_config: &CliConfig`
- `status.rs:21` - `_config: &CliConfig`
- `list_packages.rs:20` - `_config: &CliConfig`
- `logs.rs:66` - `_config: &CliConfig` (in `execute_db_mode`)

**Impact:** Low - follows interface contract but indicates potential missing functionality.

---

### 3.2 Hardcoded Logs Directory

**File:** `logs.rs:17`
```rust
const DEFAULT_LOGS_DIR: &str = "/var/www/html/tyingshoelaces/logs";
```

**Issue:** Hardcoded path that should come from profile configuration.

**Resolution:** Use `profile.paths.logs_dir()` or similar.

---

### 3.3 `--raw` Flag Not Working as Expected

**File:** `list_packages.rs:29-33`
```rust
if args.raw {
    Ok(CommandResult::copy_paste(output).with_title("MCP Packages"))
} else {
    Ok(CommandResult::list(output).with_title("MCP Packages"))
}
```

**Issue:** Both outputs render the same JSON structure. The `--raw` flag should output space-separated package names for shell scripts.

**Resolution:** Output should be `content-manager systemprompt-admin` in raw mode.

---

## 4. Enhancement Suggestions

### 4.1 Server Filtering for `mcp status`

Add ability to filter status to specific server:
```rust
#[arg(long, help = "Filter to specific server")]
pub server: Option<String>,
```

---

### 4.2 Rich Validation Output

Enhance `mcp validate` to:
1. Connect to server and list actual tools
2. Measure connection latency
3. Verify tool schemas are valid
4. Check server health endpoint if available

---

### 4.3 Log Level Filtering

Add `--level` flag to filter logs by severity:
```rust
#[arg(long, help = "Filter by log level: debug, info, warn, error")]
pub level: Option<LogLevel>,
```

---

### 4.4 Table Formatting for Non-JSON Output

Current non-JSON output dumps JSON. Consider proper table formatting:

**Current:**
```
MCP Servers
{
  "servers": [...]
}
```

**Suggested:**
```
MCP Servers

NAME               PORT   ENABLED  STATUS
content-manager    5003   true     ready
systemprompt-admin 5002   true     ready
```

---

### 4.5 Batch Validation Command

Add `mcp validate --all` to validate all configured servers at once:
```bash
sp mcp validate --all
```

---

## 5. Priority Matrix

| Item | Severity | Effort | Priority |
|------|----------|--------|----------|
| Update README for `--disabled` flag | Low | Low | P3 |
| Update README for `--server` -> `--detailed` | Medium | Low | P2 |
| Add `--timeout` flag to validate | Medium | Medium | P2 |
| Add `--level` flag to logs | Medium | Medium | P2 |
| Fix `--raw` output for list-packages | Low | Low | P3 |
| Rich validation output with tools | High | High | P1 |
| Remove hardcoded logs path | Medium | Low | P2 |
| Table formatting for human output | Low | Medium | P3 |

---

## 6. Action Items

### Immediate (Update README to Match Implementation)

1. Remove `--disabled` flag documentation from README or add to implementation
2. Change `--server` to `--detailed` in README for status command
3. Remove `--timeout` flag documentation or add to validate
4. Remove `--level` flag documentation or add to logs

### Short-term (Implementation Fixes)

1. Fix `--raw` flag in list-packages to output space-separated string
2. Replace hardcoded logs directory with profile path
3. Add server filtering to status command

### Medium-term (Enhancements)

1. Implement rich validation output with actual tool discovery
2. Add log level filtering
3. Add batch validation (`--all` flag)
4. Improve human-readable table output

---

## 7. Files Modified Since Last Commit

The MCP command files have not been modified in the current git status, indicating this module is stable but may need updates based on this audit.
