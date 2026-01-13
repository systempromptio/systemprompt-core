# Jobs Domain Plan

## Purpose
The `jobs` domain manages background jobs and scheduled tasks: listing, running, monitoring, and cleanup operations.

## CLI Structure
```
systemprompt jobs list
systemprompt jobs run <job_name>
systemprompt jobs cleanup-sessions [--hours 1]
systemprompt jobs session-cleanup [--hours 1]
systemprompt jobs log-cleanup [--days 30]
```

## Files
```
commands/jobs/
└── mod.rs       # JobsCommands enum and job operations
```

## Commands

### list
List all available registered jobs.

**Output (table):**
- Job name
- Description
- Schedule (cron expression)
- Enabled status

**Output (JSON):**
```json
[
  {
    "name": "database_cleanup",
    "description": "Clean up old database records",
    "schedule": "0 2 * * *",
    "enabled": true
  }
]
```

**Example:**
```bash
systemprompt jobs list
systemprompt --json jobs list
```

### run
Run a scheduled job manually.

**Arguments:**
- `job_name` - Name of job to execute

**Process:**
1. Look up job in inventory registry
2. Create JobContext with db pool and app context
3. Execute job
4. Report result (success/failure with message)

**Example:**
```bash
systemprompt jobs run database_cleanup
systemprompt jobs run session_cleanup
```

### cleanup-sessions
Clean up inactive sessions.

**Flags:**
- `--hours` - Hours of inactivity threshold (default: 1)

**Example:**
```bash
systemprompt jobs cleanup-sessions --hours 2
```

### session-cleanup
Alias for cleanup-sessions.

### log-cleanup
Clean up old log entries by running the database_cleanup job.

**Flags:**
- `--days` - Days to retain (default: 30)

**Example:**
```bash
systemprompt jobs log-cleanup --days 7
```

## Job Registry

Jobs are registered via the `inventory` crate using the `Job` trait:

```rust
#[async_trait]
pub trait Job: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schedule(&self) -> &'static str;
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult>;
    fn enabled(&self) -> bool;
}
```

### Available Jobs (from codebase)
- `database_cleanup` - Clean up old database records
- `cleanup_inactive_sessions` - Close inactive sessions
- `cleanup_empty_contexts` - Remove empty conversation contexts
- `feature_extraction` - Extract features from data
- `malicious_ip_blacklist` - Update IP blacklist
- `behavioral_analysis` - Run behavioral analysis

## Implementation Details

### JobContext
```rust
pub struct JobContext {
    db_pool: Arc<dyn Any + Send + Sync>,
    app_context: Arc<dyn Any + Send + Sync>,
}
```

### JobResult
```rust
pub struct JobResult {
    pub success: bool,
    pub message: Option<String>,
    pub items_processed: Option<u64>,
    pub items_failed: Option<u64>,
    pub duration_ms: u64,
}
```

## Dependencies
- `systemprompt_traits::{Job, JobContext, JobResult}`
- `systemprompt_core_analytics::SessionCleanupService`
- `systemprompt_runtime::AppContext`
- `systemprompt_generator` (for job registration)
- `inventory` crate for job discovery

## JSON Output Support

```bash
systemprompt --json jobs list
```

## Future Enhancements (not in initial scope)

These could be added later:
- `systemprompt jobs status [job_name]` - Show job execution status
- `systemprompt jobs history [job_name]` - Show execution history
- `systemprompt jobs enable <job_name>` - Enable a disabled job
- `systemprompt jobs disable <job_name>` - Disable a job
- `systemprompt jobs schedule <job_name>` - Show next scheduled run

## Error Handling

- Unknown job name: List available jobs
- Execution failure: Show error message and return non-zero exit
- Connection issues: Clear error with suggested actions
