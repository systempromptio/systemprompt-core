# Jobs Command Tech Debt Assessment

## Test Results Summary

| Command | Status | Issues |
|---------|--------|--------|
| `jobs list` | PASS | Works correctly in both modes |
| `jobs list --json` | PASS | Returns valid JSON |
| `jobs run <name>` | PARTIAL | Works but no JSON output support |
| `jobs run nonexistent` | PASS | Proper error handling |
| `jobs cleanup-sessions` | PARTIAL | Works but ignores `--json` flag |
| `jobs cleanup-sessions --hours N` | PASS | Custom hours respected |
| `jobs session-cleanup` | PASS | Alias works correctly |
| `jobs log-cleanup` | FAIL | Ignores `--days` parameter |
| `jobs log-cleanup --json` | FAIL | No JSON output |

---

## CLI README Violations

### Part 4: Artifact-Compatible Results (MANDATORY)

| Violation | Severity | Location |
|-----------|----------|----------|
| Returns `Result<()>` instead of `Result<CommandResult<T>>` | HIGH | `mod.rs:36` |
| No `types.rs` file for structured output types | HIGH | Missing file |
| Missing `JsonSchema` derive on output types | HIGH | No types defined |
| Direct `CliService` calls instead of `render_result()` | MEDIUM | Multiple locations |

**Current signature (WRONG):**
```rust
pub async fn execute(cmd: JobsCommands, config: &CliConfig) -> Result<()>
```

**Expected signature:**
```rust
pub async fn execute(cmd: JobsCommands, config: &CliConfig) -> Result<CommandResult<JobsOutput>>
```

### Part 1.3: Forbidden Patterns

| Pattern | Status | Notes |
|---------|--------|-------|
| `println!` | PASS | Not found |
| `eprintln!` | PASS | Not found |
| `unwrap()` | PASS | Not found (excluding `unwrap_or`) |
| `expect()` | PASS | Not found |
| `env::set_var` | PASS | Not found |

### Part 1.2: Dual-Mode Operation

| Issue | Severity |
|-------|----------|
| `jobs run` ignores `--json` flag | HIGH |
| `jobs cleanup-sessions` ignores `--json` flag | HIGH |
| `jobs log-cleanup` ignores `--json` flag | HIGH |

---

## Functional Bugs

### 1. `log-cleanup` Ignores `--days` Parameter (CRITICAL)

**Location:** `mod.rs:141-150`

```rust
async fn cleanup_logs(days: i32, ctx: Arc<AppContext>) -> Result<()> {
    CliService::section("Log Cleanup");
    CliService::info(&format!(
        "Cleaning up log entries older than {} day(s)...",
        days
    ));
    // BUG: days parameter is displayed but never passed to the job
    run_job("database_cleanup", ctx).await
}
```

The `--days` parameter is:
1. Accepted from the CLI
2. Displayed in the info message
3. **Never actually used** - always runs `database_cleanup` with its default config

**Expected behavior:** Should either:
- Pass `days` to the job via `JobContext`
- Call a dedicated cleanup function that respects the parameter

---

## Missing Features

### 1. No Output Types File

Commands like `content`, `analytics`, and `users` have dedicated `types.rs` files:
- `content/types.rs`
- `analytics/sessions/mod.rs` (with output types)
- `users/types.rs`

The `jobs` module lacks this structure.

### 2. Missing JSON Output for Job Execution

The README documents JSON output structure for `jobs run`:
```json
{
  "job_name": "content_ingestion",
  "status": "completed",
  "duration_seconds": 15,
  "result": {
    "success": true,
    "message": "Ingested 25 content files"
  }
}
```

But the implementation only outputs text via `CliService`.

### 3. Missing JSON Output for Cleanup Commands

The README documents:
```json
{
  "job_name": "session_cleanup",
  "sessions_cleaned": 15,
  "hours_threshold": 1,
  "message": "Cleaned up 15 inactive session(s)"
}
```

But the implementation ignores JSON mode entirely.

---

## Documentation Discrepancies

### README vs Actual Jobs

**README lists these jobs:**
- `content_ingestion`
- `session_cleanup`
- `database_cleanup`
- `publish_content`
- `sitemap_generation`
- `cache_cleanup`

**Actual registered jobs:**
- `file_ingestion`
- `cleanup_anonymous_users`
- `behavioral_analysis`
- `cleanup_empty_contexts`
- `cleanup_inactive_sessions`
- `feature_extraction`
- `database_cleanup`
- `content_ingestion`
- `publish_content`
- `image_optimization`

**Missing from README:** 6 jobs
**Missing from system:** 2 jobs (`sitemap_generation`, `cache_cleanup`)

---

## Friction Points

1. **No dry-run support** for cleanup commands - users can't preview what will be deleted
2. **No --yes flag** for cleanup operations (though they're not truly destructive)
3. **No progress indication** for long-running jobs
4. **No job history** command to see past executions
5. **No way to run multiple jobs** in sequence

---

## Recommended Fixes

### Priority 1: Critical Bugs

1. Fix `log-cleanup` to actually use the `--days` parameter
2. Add JSON output support for all commands

### Priority 2: Architecture Compliance

1. Create `types.rs` with proper output types
2. Change return type to `CommandResult<T>`
3. Use `render_result()` pattern

### Priority 3: Documentation

1. Update README to match actual job list
2. Add missing job documentation for new jobs

---

## Suggested Enhancement Implementation

### types.rs Structure

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobInfo {
    pub name: String,
    pub description: String,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobListOutput {
    pub jobs: Vec<JobInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobRunOutput {
    pub job_name: String,
    pub status: String,
    pub duration_seconds: Option<f64>,
    pub result: JobRunResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobRunResult {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionCleanupOutput {
    pub job_name: String,
    pub sessions_cleaned: i64,
    pub hours_threshold: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogCleanupOutput {
    pub job_name: String,
    pub entries_deleted: i64,
    pub days_threshold: i32,
    pub message: String,
}
```

---

## Enhancement Suggestions

### 1. Add `--dry-run` to Cleanup Commands

```bash
sp jobs cleanup-sessions --dry-run
# Would clean up 15 inactive session(s)

sp jobs log-cleanup --days 7 --dry-run
# Would delete 5000 log entries older than 7 days
```

### 2. Add Job History Command

```bash
sp jobs history
sp jobs history --job content_ingestion
sp jobs history --since 24h --json
```

### 3. Add Job Status/Details Command

```bash
sp jobs show content_ingestion
# Shows: name, description, schedule, last run, next run, enabled status
```

### 4. Add Batch Job Execution

```bash
sp jobs run --all
sp jobs run content_ingestion publish_content --sequential
```

### 5. Add Job Enable/Disable Commands

```bash
sp jobs enable content_ingestion
sp jobs disable behavioral_analysis
```

### 6. Add Next Run Time Display

```bash
sp jobs list
# name | description | schedule | enabled | next_run
```

---

## Compliance Checklist Update

Current README checklist claims compliance but is inaccurate:

```markdown
## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`  ✅ TRUE
- [x] All commands return `CommandResult<T>`              ❌ FALSE - Returns Result<()>
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`  ❌ FALSE - No types file
- [x] No `println!` / `eprintln!` - uses `CliService`    ✅ TRUE
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`  ✅ TRUE
- [x] JSON output supported via `--json` flag            ❌ PARTIAL - Only list command
```

---

## Estimated Effort

| Task | Complexity |
|------|------------|
| Create types.rs | Low |
| Refactor to CommandResult | Medium |
| Fix log-cleanup bug | Low |
| Add JSON output to all commands | Medium |
| Update README | Low |
| Add dry-run support | Medium |
| Add job history | High |
