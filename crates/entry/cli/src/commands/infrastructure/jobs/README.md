# Jobs CLI Commands

Command reference for jobs. Use the installed command’s `--help` output for its complete arguments and defaults.

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

| Command | Description | Artifact Type |
|---------|-------------|---------------|
| `infra jobs list` | List available scheduled jobs | `Table` |
| `infra jobs show <name>` | Show detailed job information | `PresentationCard` |
| `infra jobs run <name...>` | Run job(s) manually | `Table` |
| `infra jobs run --all` | Run all enabled jobs | `Table` |
| `infra jobs history` | View job execution history | `Table` |
| `infra jobs enable <name>` | Enable a job | `Text` |
| `infra jobs disable <name>` | Disable a job's scheduled runs; survives restarts. A manual `infra jobs run` still runs it | `Text` |
| `infra jobs prune [--dry-run] [--yes]` | Delete `scheduled_jobs` rows no job linked into this binary names (a job whose extension the profile disables is kept); run from a binary that links every capability the cluster runs | `Table` |

---

## Core Commands

### jobs list

List all available scheduled jobs from the registry.

```bash
sp infra jobs list
sp --json infra jobs list
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
sp --json infra jobs show database_cleanup
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

## Creating a New Job

A job is contributed by the member that owns it, from its `Extension::contribute`. The kernel builds the schedule from the admitted composition, so a job exists only in a binary whose composition admits its member, and only while the profile leaves that member enabled.

### Step 1: Implement the Job Trait

```rust
use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderResult};

#[derive(Debug, Clone, Copy)]
pub(crate) struct MyCustomJob;

#[async_trait]
impl Job for MyCustomJob {
    fn name(&self) -> &'static str {
        "my_custom_job"
    }

    fn description(&self) -> &'static str {
        "Does something useful on a schedule"
    }

    fn schedule(&self) -> &'static str {
        "0 */15 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let db_pool = ctx.get::<DbPool>()?;
        let processed = do_the_work(db_pool).await?;
        Ok(JobResult::success()
            .with_message(format!("Processed {processed} items"))
            .with_stats(processed, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}
```

### Step 2: Contribute It

```rust
impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata { /* ... */ }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.job(MyCustomJob);
        Ok(())
    }
}
```

### Step 3: Compose the Member

The binary names the member in its composition (`systemprompt::reference().with(MyExtension).run()`). The job then appears in `infra jobs list`.

---

## Job Trait Reference

```rust
#[async_trait]
pub trait Job: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str { /* defaulted */ }
    fn schedule(&self) -> &'static str;
    fn tags(&self) -> Vec<&'static str> { /* defaulted */ }
    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult>;
    fn enabled(&self) -> bool { /* defaulted */ }
    fn configured(&self, _deployment: &Dependencies) -> bool { /* defaulted */ }
    fn schedulable(&self) -> bool { /* defaulted */ }
    fn scope(&self) -> JobScope { /* defaulted */ }
    fn default_schedule(&self) -> Option<&'static str> { /* defaulted */ }
    fn db_budget(&self) -> Option<DbBudget> { /* defaulted */ }
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

Location: `<your-project>/extensions/blog/src/jobs/`

```rust
// blog_content_ingestion.rs
use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderResult};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BlogContentIngestionJob;

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

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let db_pool = ctx.get::<DbPool>()?;
        let posts_ingested = ingest_blog_posts(db_pool).await?;

        Ok(JobResult::success()
            .with_stats(posts_ingested, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}
```

The blog member contributes it with `c.job(BlogContentIngestionJob)` in its `contribute`.

---

## Workflow Examples

### Development Workflow

```bash
# List all jobs
sp --json infra jobs list | jq '.data.jobs[].name'

# Check job details before running
sp infra jobs show content_ingestion

# Run the job
sp infra jobs run content_ingestion

# Check execution history
sp --json infra jobs history --job content_ingestion
```

### Maintenance Workflow

```bash
# Run all maintenance jobs
sp infra jobs run cleanup_inactive_sessions cleanup_empty_contexts database_cleanup

# Or run everything
sp infra jobs run --all

# Check results
sp --json infra jobs history -n 10
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
