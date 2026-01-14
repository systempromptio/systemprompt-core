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

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `jobs list` | List available scheduled jobs | `Table` | No |
| `jobs run <name>` | Run a scheduled job manually | `Text` | No (DB only) |
| `jobs cleanup-sessions` | Clean up inactive sessions | `Text` | No (DB only) |
| `jobs session-cleanup` | Clean up inactive sessions (alias) | `Text` | No (DB only) |
| `jobs log-cleanup` | Clean up old log entries | `Text` | No (DB only) |

---

## Core Commands

### jobs list

List all available scheduled jobs from the registry.

```bash
sp jobs list
sp --json jobs list
```

**Output Structure:**
```json
{
  "jobs": [
    {
      "name": "content_ingestion",
      "description": "Ingest markdown content from configured directories",
      "schedule": "0 0 * * * *",
      "enabled": true
    },
    {
      "name": "session_cleanup",
      "description": "Clean up expired user sessions",
      "schedule": "0 */15 * * * *",
      "enabled": true
    },
    {
      "name": "database_cleanup",
      "description": "Clean up old database entries",
      "schedule": "0 0 2 * * *",
      "enabled": true
    },
    {
      "name": "publish_content",
      "description": "Full publishing pipeline",
      "schedule": "manual",
      "enabled": true
    }
  ],
  "total": 4
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `description`, `schedule`, `enabled`

**Schedule Format (Cron):**
- `0 0 * * * *` - Every hour at minute 0
- `0 */15 * * * *` - Every 15 minutes
- `0 0 2 * * *` - Daily at 2:00 AM
- `manual` - Only run manually via CLI

---

### jobs run

Run a scheduled job manually.

```bash
sp jobs run <job-name>
sp jobs run content_ingestion
sp jobs run session_cleanup
sp jobs run database_cleanup
sp jobs run publish_content
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Job name to run |

**Output Structure:**
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

**Artifact Type:** `Text`

---

### jobs cleanup-sessions

Clean up inactive user sessions.

```bash
sp jobs cleanup-sessions
sp jobs cleanup-sessions --hours 2
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--hours` | `1` | Sessions inactive for more than N hours |

**Output Structure:**
```json
{
  "job_name": "session_cleanup",
  "sessions_cleaned": 15,
  "hours_threshold": 1,
  "message": "Cleaned up 15 inactive session(s)"
}
```

**Artifact Type:** `Text`

---

### jobs session-cleanup

Alias for `cleanup-sessions`.

```bash
sp jobs session-cleanup
sp jobs session-cleanup --hours 4
```

---

### jobs log-cleanup

Clean up old log entries.

```bash
sp jobs log-cleanup
sp jobs log-cleanup --days 7
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--days` | `30` | Delete logs older than N days |

**Output Structure:**
```json
{
  "job_name": "log_cleanup",
  "entries_deleted": 5000,
  "days_threshold": 30,
  "message": "Deleted 5000 log entries older than 30 days"
}
```

**Artifact Type:** `Text`

---

## Available Jobs Reference

| Job Name | Description | Schedule |
|----------|-------------|----------|
| `content_ingestion` | Ingest markdown files into database | Hourly |
| `session_cleanup` | Remove expired sessions | Every 15 min |
| `database_cleanup` | Clean old database entries | Daily 2 AM |
| `publish_content` | Full publishing pipeline | Manual |
| `sitemap_generation` | Regenerate sitemap.xml | Hourly |
| `cache_cleanup` | Clear expired cache entries | Every 30 min |

---

## Complete Jobs Workflow Example

This flow demonstrates common job operations:

```bash
# Phase 1: List available jobs
sp --json jobs list

# Phase 2: Run content ingestion
sp jobs run content_ingestion

# Phase 3: Run full publish pipeline
sp jobs run publish_content

# Phase 4: Clean up sessions
sp jobs cleanup-sessions --hours 2

# Phase 5: Clean up logs
sp jobs log-cleanup --days 7

# Phase 6: Verify cleanup
sp --json analytics sessions stats
```

---

## Scheduled Job Cron Examples

```
# Every minute
* * * * * *

# Every 5 minutes
*/5 * * * * *

# Every hour at minute 0
0 * * * * *

# Daily at midnight
0 0 * * * *

# Daily at 2:30 AM
30 2 * * * *

# Weekly on Sunday at midnight
0 0 * * 0 *

# Monthly on the 1st at midnight
0 0 1 * * *
```

---

## Job Integration with Content Pipeline

```bash
# Step 1: Create content files in markdown
mkdir -p /services/content/blog
cat << 'EOF' > /services/content/blog/my-post.md
---
title: My Blog Post
slug: my-post
---
Content here...
EOF

# Step 2: Run content ingestion
sp jobs run content_ingestion

# Step 3: Verify content was ingested
sp content list --source blog

# Step 4: Run full publish (images, prerender, sitemap)
sp jobs run publish_content

# Step 5: Verify sitemap was generated
cat /services/web/dist/sitemap.xml
```

---

## Error Handling

### Job Not Found

```bash
sp jobs run nonexistent
# Error: Unknown job: nonexistent
# Use 'jobs list' to see available jobs
```

### Job Failed

```bash
sp jobs run content_ingestion
# Error: Job failed: Failed to connect to database
```

### Database Connection Error

```bash
sp jobs cleanup-sessions
# Error: Failed to initialize application context. Check database connection.
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json jobs list | jq .

# Extract specific fields
sp --json jobs list | jq '.jobs[].name'
sp --json jobs list | jq '.jobs[] | select(.enabled == true)'
sp --json jobs list | jq '.jobs[] | select(.schedule != "manual")'

# Get job count
sp --json jobs list | jq '.total'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
