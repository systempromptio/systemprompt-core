# Jobs CLI Commands

This document provides complete documentation for AI agents to use the jobs CLI commands. All commands support non-interactive mode for automation.

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

| Command | Description | Artifact Type |
|---------|-------------|---------------|
| `infra jobs list` | List available scheduled jobs | `Table` |
| `infra jobs show <name>` | Show detailed job information | `PresentationCard` |
| `infra jobs run <name...>` | Run job(s) manually | `Table` |
| `infra jobs run --all` | Run all enabled jobs | `Table` |
| `infra jobs history` | View job execution history | `Table` |
| `infra jobs enable <name>` | Enable a job | `Text` |
| `infra jobs disable <name>` | Disable a job | `Text` |
| `infra jobs cleanup-sessions` | Clean up inactive sessions | `Text` |
| `infra jobs log-cleanup` | Clean up old log entries | `Text` |

---

## Core Commands

### jobs list

List all available scheduled jobs from the registry.

```bash
sp infra jobs list
sp --json jobs list
```

**Output Structure:**
```json
{
  "data": {
    "jobs": [
      {
        "name": "content_ingestion",
        "description": "Ingests markdown content from configured directories",
        "schedule": "0 0 * * * *",
        "enabled": true
      }
    ],
    "total": 10
  },
  "artifact_type": "table",
  "title": "Available Jobs"
}
```

---

### jobs show

Show detailed information about a specific job.

```bash
sp infra jobs show content_ingestion
sp --json jobs show database_cleanup
```

**Output Structure:**
```json
{
  "data": {
    "name": "content_ingestion",
    "description": "Ingests markdown content from configured directories",
    "schedule": "0 0 * * * *",
    "schedule_human": "Every hour",
    "enabled": true,
    "last_run": "2026-01-14T10:00:00Z",
    "next_run": "2026-01-14T11:00:00Z",
    "last_status": "success",
    "last_error": null,
    "run_count": 42
  },
  "artifact_type": "presentation_card",
  "title": "Job: content_ingestion"
}
```

---

### jobs run

Run one or more scheduled jobs manually.

```bash
# Run a single job
sp infra jobs run content_ingestion

# Run multiple jobs
sp infra jobs run content_ingestion publish_content database_cleanup

# Run all enabled jobs
sp infra jobs run --all
```

**Arguments & Flags:**
| Argument/Flag | Description |
|---------------|-------------|
| `<name...>` | Job name(s) to run |
| `--all` | Run all enabled jobs |
| `--sequential` | Run jobs one at a time (default: parallel) |

**Output Structure:**
```json
{
  "data": {
    "jobs_run": [
      {
        "job_name": "content_ingestion",
        "status": "success",
        "duration_ms": 64,
        "result": {
          "success": true,
          "message": "Ingested 31 files",
          "items_processed": 31,
          "items_failed": 0
        }
      }
    ],
    "total": 1,
    "succeeded": 1,
    "failed": 0
  },
  "artifact_type": "table",
  "title": "Job Execution Results"
}
```

---

### jobs history

View job execution history.

```bash
sp infra jobs history
sp infra jobs history --job content_ingestion
sp infra jobs history --status failed
sp infra jobs history -n 50
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--job` | | Filter by job name |
| `--status` | | Filter by status (success, failed, running) |
| `-n, --limit` | `20` | Number of entries to show |

**Output Structure:**
```json
{
  "data": {
    "entries": [
      {
        "job_name": "publish_content",
        "status": "success",
        "run_at": "2026-01-14T10:30:02Z",
        "error": null
      }
    ],
    "total": 5
  },
  "artifact_type": "table",
  "title": "Job Execution History"
}
```

---

### jobs enable / disable

Enable or disable a job.

```bash
sp infra jobs enable behavioral_analysis
sp infra jobs disable behavioral_analysis
```

**Output Structure:**
```json
{
  "data": {
    "job_name": "behavioral_analysis",
    "enabled": true,
    "message": "Job 'behavioral_analysis' has been enabled"
  },
  "artifact_type": "text",
  "title": "Job Enabled"
}
```

---

### jobs cleanup-sessions

Clean up inactive user sessions.

```bash
sp infra jobs cleanup-sessions
sp infra jobs cleanup-sessions --hours 2
sp infra jobs cleanup-sessions --dry-run
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--hours` | `1` | Sessions inactive for more than N hours |
| `--dry-run` | | Preview without executing |

**Output Structure:**
```json
{
  "data": {
    "job_name": "session_cleanup",
    "sessions_cleaned": 15,
    "hours_threshold": 1,
    "message": "Cleaned up 15 inactive session(s)"
  },
  "artifact_type": "text",
  "title": "Session Cleanup"
}
```

---

### jobs log-cleanup

Clean up old log entries.

```bash
sp infra jobs log-cleanup
sp infra jobs log-cleanup --days 7
sp infra jobs log-cleanup --days 7 --dry-run
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--days` | `30` | Delete logs older than N days |
| `--dry-run` | | Preview without executing |

**Output Structure:**
```json
{
  "data": {
    "job_name": "log_cleanup",
    "entries_deleted": 5000,
    "days_threshold": 30,
    "message": "Deleted 5000 log entries older than 30 days"
  },
  "artifact_type": "text",
  "title": "Log Cleanup"
}
```

---

## Creating a New Job

Jobs are registered at compile-time using the `inventory` crate. To create a new job:

### Step 1: Create the Job File

Create a new file in your extension or domain crate:

```
/var/www/html/tyingshoelaces/extensions/blog/src/jobs/my_job.rs
```

### Step 2: Implement the Job Trait

```rust
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::info;

/// My custom job that does something useful
#[derive(Debug, Clone, Copy)]
pub struct MyCustomJob;

#[async_trait]
impl Job for MyCustomJob {
    /// Unique job identifier (used in CLI commands)
    fn name(&self) -> &'static str {
        "my_custom_job"
    }

    /// Human-readable description
    fn description(&self) -> &'static str {
        "Does something useful on a schedule"
    }

    /// Cron schedule (6 fields: sec min hour day month weekday)
    fn schedule(&self) -> &'static str {
        "0 */15 * * * *"  // Every 15 minutes
    }

    /// Whether this job is enabled by default
    fn enabled(&self) -> bool {
        true
    }

    /// The actual job logic
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start = std::time::Instant::now();

        // Extract database pool from context
        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available"))?
        );

        info!("my_custom_job started");

        // Do your work here
        let processed = do_the_work(&db_pool).await?;

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            processed = processed,
            duration_ms = duration_ms,
            "my_custom_job completed"
        );

        // Return result with stats
        Ok(JobResult::success()
            .with_message(format!("Processed {} items", processed))
            .with_stats(processed, 0)
            .with_duration(duration_ms))
    }
}

async fn do_the_work(pool: &DbPool) -> Result<u64> {
    // Your implementation here
    Ok(42)
}

// CRITICAL: Register the job with inventory
systemprompt_traits::submit_job!(&MyCustomJob);
```

### Step 3: Export from Module

In your `jobs/mod.rs`:

```rust
mod my_job;

pub use my_job::MyCustomJob;
```

### Step 4: Include in Crate

Ensure the jobs module is included in your crate's `lib.rs`:

```rust
pub mod jobs;
```

### Step 5: Link to CLI

The job will automatically appear in `infra jobs list` after rebuilding, as long as the crate is linked to the CLI binary. For extensions, this happens through the generator crate dependency.

---

## Job Trait Reference

```rust
#[async_trait]
pub trait Job: Send + Sync + 'static {
    /// Unique identifier for the job
    fn name(&self) -> &'static str;

    /// Human-readable description (optional, defaults to "")
    fn description(&self) -> &'static str { "" }

    /// Cron schedule expression (6 fields)
    fn schedule(&self) -> &'static str;

    /// Execute the job
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult>;

    /// Whether the job is enabled (optional, defaults to true)
    fn enabled(&self) -> bool { true }
}
```

---

## JobResult Builder

```rust
// Success with message
JobResult::success()
    .with_message("Completed successfully")

// Success with stats
JobResult::success()
    .with_stats(100, 5)  // processed, failed
    .with_duration(1500) // milliseconds

// Failure
JobResult::failure("Database connection failed")
```

---

## Cron Schedule Format

The schedule uses 6-field cron syntax:

```
┌──────────── second (0-59)
│ ┌────────── minute (0-59)
│ │ ┌──────── hour (0-23)
│ │ │ ┌────── day of month (1-31)
│ │ │ │ ┌──── month (1-12)
│ │ │ │ │ ┌── day of week (0-6, Sun=0)
│ │ │ │ │ │
* * * * * *
```

**Common Schedules:**
| Schedule | Description |
|----------|-------------|
| `0 0 * * * *` | Every hour |
| `0 */15 * * * *` | Every 15 minutes |
| `0 */30 * * * *` | Every 30 minutes |
| `0 0 */2 * * *` | Every 2 hours |
| `0 0 3 * * *` | Daily at 3:00 AM |
| `0 30 2 * * *` | Daily at 2:30 AM |
| `0 0 0 * * 0` | Weekly on Sunday at midnight |
| `0 0 0 1 * *` | Monthly on the 1st at midnight |

---

## Example: Blog Extension Job

Location: `/var/www/html/tyingshoelaces/extensions/blog/src/jobs/`

```rust
// blog_content_ingestion.rs
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};

#[derive(Debug, Clone, Copy)]
pub struct BlogContentIngestionJob;

#[async_trait]
impl Job for BlogContentIngestionJob {
    fn name(&self) -> &'static str {
        "blog_content_ingestion"
    }

    fn description(&self) -> &'static str {
        "Ingests blog posts from markdown files"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"  // Every hour
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start = std::time::Instant::now();
        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available"))?
        );

        // Ingest blog content...
        let posts_ingested = ingest_blog_posts(&db_pool).await?;

        Ok(JobResult::success()
            .with_stats(posts_ingested, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

systemprompt_traits::submit_job!(&BlogContentIngestionJob);
```

---

## Workflow Examples

### Development Workflow

```bash
# List all jobs
sp --json jobs list | jq '.data.jobs[].name'

# Check job details before running
sp infra jobs show content_ingestion

# Preview cleanup without executing
sp infra jobs cleanup-sessions --dry-run
sp infra jobs log-cleanup --days 7 --dry-run

# Run the job
sp infra jobs run content_ingestion

# Check execution history
sp --json jobs history --job content_ingestion
```

### Maintenance Workflow

```bash
# Run all maintenance jobs
sp infra jobs run cleanup_inactive_sessions cleanup_empty_contexts database_cleanup

# Or run everything
sp infra jobs run --all

# Check results
sp --json jobs history -n 10
```

### Disable/Enable Workflow

```bash
# Temporarily disable a job
sp infra jobs disable behavioral_analysis

# Re-enable when ready
sp infra jobs enable behavioral_analysis

# Verify status
sp infra jobs show behavioral_analysis
```

---

## Database Schema

Jobs are tracked in the `scheduled_jobs` table:

```sql
CREATE TABLE scheduled_jobs (
    id TEXT PRIMARY KEY,
    job_name TEXT NOT NULL UNIQUE,
    schedule TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    last_run TIMESTAMPTZ,
    next_run TIMESTAMPTZ,
    last_status TEXT,          -- 'success', 'failed', 'running'
    last_error TEXT,
    run_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Cleanup commands support `--dry-run`
