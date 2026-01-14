# Logs CLI Tech Debt & Enhancement Suggestions

## Summary

The logs CLI module is **mostly compliant** with the CLI README standards. All commands are functional, use proper `CommandResult` types, and follow the forbidden pattern rules. However, there are documentation discrepancies, inconsistencies, and missing workflows.

---

## Compliance Status

### Passing

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` command requires `--yes` / `-y` flag
- [x] `cleanup` command requires `--older-than` or `--keep-last-days`
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Non-interactive mode errors are clear

### Issues Found

See details below.

---

## README vs Code Discrepancies

### Default Value Mismatches

| Command | README Says | Code Actually Uses |
|---------|-------------|-------------------|
| `logs view --tail` | 50 | **20** |
| `logs search --limit` | 100 | **50** |
| `logs trace list --limit` | 50 | **20** |
| `logs request list --limit` | 50 | **20** |

**Fix Required:** Update README to reflect actual defaults, or update code to match documented defaults.

---

## Inconsistencies

### Missing `-n` Shortcut on `logs request list`

Most limit flags have a `-n` shortcut for consistency:

| Command | Has `-n`? |
|---------|-----------|
| `logs view --tail` | Yes (`-n`) |
| `logs search --limit` | Yes (`-n`) |
| `logs trace list --limit` | Yes (`-n`) |
| `logs request list --limit` | **No** |

**Fix:** Add `-n` alias to `logs request list --limit` for consistency.

```rust
// In request/list.rs
#[arg(
    long,
    short = 'n',  // ADD THIS
    default_value = "20",
    help = "Maximum number of requests to return"
)]
pub limit: i64,
```

---

## Missing Documentation: Agent Message Tracing Workflow

The logs README doesn't explain how to trace agent messages through the logging system. Users who send messages via `agents message` don't know how to find the corresponding trace.

### Recommended Addition to README

Add a new section:

```markdown
## Tracing Agent Messages

When you send a message to an agent, you can trace the full execution:

### Step 1: Send Message and Get Task ID

```bash
RESPONSE=$(sp --json agents message admin -m "What is 2+2?" --token "$TOKEN" --blocking)
TASK_ID=$(echo "$RESPONSE" | jq -r '.data.task.task_id')
echo "Task ID: $TASK_ID"
```

### Step 2: Find the Trace

Task IDs and trace IDs are often correlated. Search for traces:

```bash
# List recent traces
sp logs trace list --since 1h

# Or search logs for the task ID
sp logs search "$TASK_ID"
```

### Step 3: View Trace Details

```bash
sp logs trace show <trace-id> --all
```

This shows:
- Execution steps
- AI requests made
- MCP tool calls
- Artifacts generated

### Step 4: Inspect Specific AI Request

```bash
sp logs request list --since 1h
sp logs request show <request-id> --messages --tools
```
```

---

## Potential Bug: Search Command

### Observation

`logs search "completed"` returned no results, while `logs view` showed messages containing "completed":

```bash
# This shows logs with "completed" in the message
sp logs view --tail 5
# Output: "Publish content job completed"

# This returns no results
sp logs search "completed"
# Output: "No matching logs found"
```

### Root Cause Analysis

Looking at `search.rs:53`:
```rust
let pattern = format!("%{}%", args.pattern);
```

And the query uses `ILIKE`, which should work. However, the search queries the database directly while `view` uses `LoggingMaintenanceService::get_recent_logs()` which may be reading from a different source or cache.

**Needs Investigation:** Verify both commands query the same data source.

---

## Tech Debt Items

### 1. Trace Status Shows "unknown"

Many traces show `status: "unknown"` which isn't informative:

```json
{
  "trace_id": "trace_8b5d5865-...",
  "status": "unknown",
  "ai_requests": 0,
  "mcp_calls": 0
}
```

**Root Cause:** In `trace/list.rs:157`:
```rust
status: r.status.unwrap_or_else(|| "unknown".to_string()),
```

The status comes from `agent_tasks` table, but if no task is linked, it defaults to "unknown".

**Suggestion:** Consider showing trace-specific status based on log events (e.g., "completed" if trace has end event, "error" if has error events).

### 2. Code Duplication in SQL Queries

`search.rs`, `export.rs`, and `request/list.rs` have similar SQL query patterns with conditional filters that result in multiple query branches.

**Example in search.rs (lines 55-145):** Four nearly identical queries for different filter combinations.

**Suggestion:** Consider using a query builder pattern or dynamic SQL to reduce duplication.

### 3. Missing `--vacuum` Flag Documentation

The README mentions `--vacuum` for `logs delete`:

```markdown
| `--vacuum` | `true` | Run VACUUM after deletion |
```

But the `DeleteArgs` struct in `delete.rs` doesn't have this flag:

```rust
pub struct DeleteArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
    // No --vacuum flag
}
```

**Fix:** Either add the flag to the code or remove from README.

---

## Enhancement Suggestions

### 1. Add `logs trace show <task-id>` Direct Support

Currently, users need to know the trace_id. Add support to look up by task_id:

```rust
// In trace/show.rs, the code already does this:
if let Ok(task_id) = ai_service.resolve_task_id(&args.id).await {
    return execute_ai_trace(&ai_service, &task_id, &args).await;
}
```

Document this in the README so users know they can use task IDs directly.

### 2. Add `--format` to `logs view`

Currently `logs view` only outputs human-readable format or full JSON. Consider adding:
- `--format table` - tabular output
- `--format compact` - one-line per log

### 3. Add `logs summary` Command

Quick stats command showing:
- Total logs by level (errors, warnings, info)
- Top modules by log volume
- Time range of available logs
- Database size

### 4. Add `logs request stats` Command

Aggregate AI request statistics:
- Total tokens used
- Total cost
- Requests by model/provider
- Average latency

### 5. Cross-Reference Documentation

Add links between:
- `agents/README.md` → `logs/README.md` for tracing
- `logs/README.md` → `agents/README.md` for message context

---

## Test Commands Summary

All commands tested and functional:

| Command | Status | Notes |
|---------|--------|-------|
| `logs view` | Pass | All filters work |
| `logs view --json` | Pass | Proper artifact type |
| `logs search` | Investigate | May not match `view` results |
| `logs stream` | Pass | Properly rejects JSON mode |
| `logs export --format json` | Pass | |
| `logs export --format csv` | Pass | |
| `logs export -o file` | Pass | |
| `logs cleanup --dry-run` | Pass | |
| `logs cleanup` (no flags) | Pass | Proper error |
| `logs delete` (no --yes) | Pass | Proper error |
| `logs trace list` | Pass | |
| `logs trace show` | Pass | Handles missing traces gracefully |
| `logs request list` | Pass | Missing `-n` shortcut |
| `logs request show` | Pass | Messages and tools work |

---

## Priority Recommendations

### High Priority (Fix Before Release)
1. Fix README default value discrepancies
2. Add `-n` shortcut to `logs request list`

### Medium Priority (Tech Debt)
3. Add agent message tracing workflow to README
4. Investigate search vs view data source discrepancy
5. Add or remove `--vacuum` flag consistently

### Low Priority (Enhancements)
6. Add `logs summary` command
7. Add `logs request stats` command
8. Reduce SQL query duplication
