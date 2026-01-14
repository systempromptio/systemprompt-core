# Logs CLI Commands

This document provides complete documentation for AI agents to use the logs CLI commands. All commands support non-interactive mode for automation.

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
| `logs view` | View log entries | `Table` | No (DB only) |
| `logs search <query>` | Search logs by pattern | `Table` | No (DB only) |
| `logs stream` | Stream logs in real-time | `Text` | No (DB only) |
| `logs export` | Export logs to file | `Text` | No (DB only) |
| `logs cleanup` | Clean up old log entries | `Text` | No (DB only) |
| `logs delete` | Delete all log entries | `Text` | No (DB only) |
| `logs trace list` | List execution traces | `Table` | No (DB only) |
| `logs trace view <id>` | View specific trace | `Card` | No (DB only) |
| `logs trace ai <id>` | View AI requests in trace | `Table` | No (DB only) |
| `logs request list` | List AI requests | `Table` | No (DB only) |
| `logs request show <id>` | Show AI request details | `Card` | No (DB only) |

---

## Core Commands

### logs view

View log entries with filtering.

```bash
sp logs view
sp --json logs view
sp logs view --tail 100
sp logs view --level error
sp logs view --since 1h
sp logs view --module agent
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--tail`, `-n` | `50` | Number of lines to show |
| `--level` | All | Filter by level: `debug`, `info`, `warn`, `error` |
| `--since` | None | Time filter (e.g., `1h`, `24h`, `7d`) |
| `--module` | None | Filter by module name |

**Output Structure:**
```json
{
  "logs": [
    {
      "timestamp": "2024-01-15T10:30:00Z",
      "level": "INFO",
      "module": "agent",
      "message": "Task completed successfully",
      "metadata": {"task_id": "task_abc123"}
    }
  ],
  "total": 50,
  "filters": {
    "level": null,
    "module": null,
    "since": null,
    "tail": 50
  }
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `level`, `module`, `message`

---

### logs search

Search logs by pattern.

```bash
sp logs search <pattern>
sp --json logs search "error"
sp logs search "timeout" --level error
sp logs search "agent" --since 1h
sp logs search "failed" --module database
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<pattern>` | Yes | Search pattern (regex supported) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--level` | All | Filter by level |
| `--since` | None | Time filter |
| `--module` | None | Filter by module |
| `--limit` | `100` | Maximum results |

**Output Structure:**
```json
{
  "logs": [
    {
      "timestamp": "2024-01-15T10:30:00Z",
      "level": "ERROR",
      "module": "agent",
      "message": "Connection timeout after 30s",
      "metadata": {}
    }
  ],
  "pattern": "timeout",
  "total": 5
}
```

**Artifact Type:** `Table`

---

### logs stream

Stream logs in real-time (like `tail -f`).

```bash
sp logs stream
sp logs stream --level error
sp logs stream --module agent
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--level` | All | Filter by level |
| `--module` | None | Filter by module |

**Output:**
Continuously streams log entries to stdout. Press Ctrl+C to stop.

**Artifact Type:** `Text`

---

### logs export

Export logs to a file.

```bash
sp logs export --format json
sp logs export --format csv --since 24h
sp logs export --format json -o ./logs-export.json
sp logs export --format csv --since 7d --level error -o ./errors.csv
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--format` | `json` | Export format: `json`, `csv` |
| `--since` | `24h` | Time range |
| `-o`, `--output` | stdout | Output file path |
| `--level` | All | Filter by level |

**Output Structure:**
```json
{
  "exported_count": 1500,
  "format": "json",
  "file_path": "./logs-export.json"
}
```

**Artifact Type:** `Text`

---

### logs cleanup

Clean up old log entries.

```bash
sp logs cleanup
sp logs cleanup --days 7
sp logs cleanup --days 30 --dry-run
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--days` | `30` | Delete logs older than N days |
| `--dry-run` | `false` | Preview without deleting |

**Output Structure:**
```json
{
  "deleted_count": 5000,
  "dry_run": false,
  "cutoff_date": "2023-12-15T00:00:00Z",
  "vacuum_performed": true
}
```

**Artifact Type:** `Text`

---

### logs delete

Delete all log entries.

```bash
sp logs delete --yes
sp logs delete --yes --vacuum
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm deletion |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--vacuum` | `true` | Run VACUUM after deletion |

**Output Structure:**
```json
{
  "deleted_count": 15000,
  "vacuum_performed": true
}
```

**Artifact Type:** `Text`

---

## Trace Commands

### logs trace list

List execution traces for debugging.

```bash
sp logs trace list
sp --json logs trace list
sp logs trace list --limit 20
sp logs trace list --since 1h
sp logs trace list --agent primary
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `50` | Maximum results |
| `--since` | `24h` | Time filter |
| `--agent` | None | Filter by agent name |

**Output Structure:**
```json
{
  "traces": [
    {
      "trace_id": "trace_abc123",
      "agent_name": "primary",
      "task_id": "task_xyz789",
      "status": "completed",
      "duration_ms": 1250,
      "ai_requests": 3,
      "tool_calls": 5,
      "started_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 50
}
```

**Artifact Type:** `Table`
**Columns:** `trace_id`, `agent_name`, `status`, `duration_ms`, `ai_requests`, `started_at`

---

### logs trace view

View detailed execution trace.

```bash
sp logs trace view <trace-id>
sp --json logs trace view trace_abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Trace ID |

**Output Structure:**
```json
{
  "trace_id": "trace_abc123",
  "agent_name": "primary",
  "task_id": "task_xyz789",
  "context_id": "ctx_abc",
  "status": "completed",
  "started_at": "2024-01-15T10:30:00Z",
  "ended_at": "2024-01-15T10:30:01.250Z",
  "duration_ms": 1250,
  "input": {
    "message": "What files are in the current directory?"
  },
  "output": {
    "response": "The current directory contains..."
  },
  "events": [
    {"type": "ai_request", "timestamp": "2024-01-15T10:30:00.100Z", "data": {}},
    {"type": "tool_call", "timestamp": "2024-01-15T10:30:00.500Z", "tool": "list_files"},
    {"type": "ai_request", "timestamp": "2024-01-15T10:30:00.900Z", "data": {}}
  ],
  "ai_requests_count": 2,
  "tool_calls_count": 1,
  "total_tokens": 1500,
  "total_cost_cents": 15
}
```

**Artifact Type:** `Card`

---

### logs trace ai

View AI requests within a trace.

```bash
sp logs trace ai <trace-id>
sp --json logs trace ai trace_abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Trace ID |

**Output Structure:**
```json
{
  "trace_id": "trace_abc123",
  "requests": [
    {
      "request_id": "req_001",
      "provider": "anthropic",
      "model": "claude-3-5-sonnet-20241022",
      "input_tokens": 500,
      "output_tokens": 200,
      "latency_ms": 850,
      "cost_cents": 7,
      "cached": false,
      "timestamp": "2024-01-15T10:30:00.100Z"
    }
  ],
  "total_requests": 2,
  "total_tokens": 1500,
  "total_cost_cents": 15
}
```

**Artifact Type:** `Table`
**Columns:** `request_id`, `model`, `tokens`, `latency_ms`, `cost_cents`, `cached`

---

## Request Commands

### logs request list

List AI requests.

```bash
sp logs request list
sp --json logs request list
sp logs request list --limit 20
sp logs request list --since 1h
sp logs request list --provider anthropic
sp logs request list --model claude-3-5-sonnet-20241022
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `50` | Maximum results |
| `--since` | `24h` | Time filter |
| `--provider` | None | Filter by provider |
| `--model` | None | Filter by model |

**Output Structure:**
```json
{
  "requests": [
    {
      "request_id": "req_abc123",
      "provider": "anthropic",
      "model": "claude-3-5-sonnet-20241022",
      "input_tokens": 500,
      "output_tokens": 200,
      "latency_ms": 850,
      "cost_cents": 7,
      "status": "success",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 50
}
```

**Artifact Type:** `Table`
**Columns:** `request_id`, `provider`, `model`, `tokens`, `latency_ms`, `cost_cents`, `status`

---

### logs request show

Show detailed AI request.

```bash
sp logs request show <request-id>
sp --json logs request show req_abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Request ID |

**Output Structure:**
```json
{
  "request_id": "req_abc123",
  "trace_id": "trace_xyz789",
  "provider": "anthropic",
  "model": "claude-3-5-sonnet-20241022",
  "input_tokens": 500,
  "output_tokens": 200,
  "total_tokens": 700,
  "latency_ms": 850,
  "cost_cents": 7,
  "status": "success",
  "cached": false,
  "cache_creation_input_tokens": 0,
  "cache_read_input_tokens": 0,
  "input_preview": "System: You are a helpful assistant...",
  "output_preview": "I'd be happy to help you with...",
  "created_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

## Complete Logs Management Flow Example

```bash
# Phase 1: View recent logs
sp --json logs view --tail 20

# Phase 2: Search for errors
sp --json logs search "error" --since 24h

# Phase 3: View specific module logs
sp --json logs view --module agent --level warn

# Phase 4: Export logs for analysis
sp logs export --format json --since 7d -o ./weekly-logs.json

# Phase 5: Trace debugging
sp --json logs trace list --since 1h
sp --json logs trace view trace_abc123
sp --json logs trace ai trace_abc123

# Phase 6: AI request analysis
sp --json logs request list --since 24h --provider anthropic
sp --json logs request show req_abc123

# Phase 7: Cleanup old logs
sp logs cleanup --days 30 --dry-run
sp logs cleanup --days 30
```

---

## Log Level Reference

| Level | Description |
|-------|-------------|
| `debug` | Detailed debugging information |
| `info` | General informational messages |
| `warn` | Warning messages for potential issues |
| `error` | Error messages for failures |

---

## Time Range Format

| Format | Description |
|--------|-------------|
| `1h` | 1 hour |
| `24h` | 24 hours |
| `7d` | 7 days |
| `30d` | 30 days |
| ISO datetime | e.g., `2024-01-15T00:00:00Z` |

---

## Error Handling

### Missing Required Flags

```bash
sp logs delete
# Error: --yes is required to delete logs in non-interactive mode
```

### Trace Not Found

```bash
sp logs trace view nonexistent
# Error: Trace 'nonexistent' not found
```

### Request Not Found

```bash
sp logs request show nonexistent
# Error: Request 'nonexistent' not found
```

### Invalid Pattern

```bash
sp logs search "[invalid"
# Error: Invalid regex pattern
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json logs view | jq .

# Extract specific fields
sp --json logs view | jq '.logs[].message'
sp --json logs trace list | jq '.traces[] | {trace_id, duration_ms}'
sp --json logs request list | jq '.requests[] | select(.cost_cents > 10)'

# Filter by criteria
sp --json logs view | jq '.logs[] | select(.level == "ERROR")'
sp --json logs trace list | jq '.traces[] | select(.status == "failed")'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` command requires `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Time range filters consistent across commands
