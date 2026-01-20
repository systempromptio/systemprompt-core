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
| `infra logs view` | View log entries | `Table` | No (DB only) |
| `infra logs search <query>` | Search logs by pattern | `Table` | No (DB only) |
| `infra logs stream` | Stream logs in real-time | `Text` | No (DB only) |
| `infra logs export` | Export logs to file | `Text` | No (DB only) |
| `infra logs cleanup` | Clean up old log entries | `Card` | No (DB only) |
| `infra logs delete` | Delete all log entries | `Card` | No (DB only) |
| `infra logs summary` | Show logs summary statistics | `Card` | No (DB only) |
| `infra logs trace list` | List execution traces | `Table` | No (DB only) |
| `infra logs trace show <id>` | View specific trace | `Card` | No (DB only) |
| `infra logs request list` | List AI requests | `Table` | No (DB only) |
| `infra logs request show <id>` | Show AI request details | `Card` | No (DB only) |
| `infra logs request stats` | Show aggregate AI statistics | `Card` | No (DB only) |

---

## Core Commands

### logs view

View log entries with filtering.

```bash
sp infra logs view
sp --json logs view
sp infra logs view --tail 100
sp infra logs view --level error
sp infra logs view --since 1h
sp infra logs view --module agent
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--tail`, `-n` | `20` | Number of lines to show |
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
  "total": 20,
  "filters": {
    "level": null,
    "module": null,
    "since": null,
    "tail": 20
  }
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `level`, `module`, `message`

---

### logs search

Search logs by pattern.

```bash
sp infra logs search <pattern>
sp --json logs search "error"
sp infra logs search "timeout" --level error
sp infra logs search "agent" --since 1h
sp infra logs search "failed" --module database
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<pattern>` | Yes | Search pattern (ILIKE match) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--level` | All | Filter by level |
| `--since` | None | Time filter |
| `--module` | None | Filter by module |
| `--limit`, `-n` | `50` | Maximum results |

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
sp infra logs stream
sp infra logs stream --level error
sp infra logs stream --module agent
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--level` | All | Filter by level |
| `--module` | None | Filter by module |
| `--interval` | `1000` | Polling interval in milliseconds |
| `--clear` | `false` | Clear screen between updates |

**Output:**
Continuously streams log entries to stdout. Press Ctrl+C to stop.

**Note:** JSON output mode (`--json`) is not supported in streaming mode.

**Artifact Type:** `Text`

---

### logs export

Export logs to a file.

```bash
sp infra logs export --format json
sp infra logs export --format csv --since 24h
sp infra logs export --format json -o ./logs-export.json
sp infra logs export --format csv --since 7d --level error -o ./errors.csv
sp infra logs export --format jsonl --limit 1000
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--format`, `-f` | `json` | Export format: `json`, `csv`, `jsonl` |
| `--since` | None | Time range filter |
| `-o`, `--output` | stdout | Output file path |
| `--level` | All | Filter by level |
| `--limit` | `10000` | Maximum logs to export |

**Output Structure:**
```json
{
  "exported_count": 1500,
  "format": "json",
  "file_path": "./logs-export.json"
}
```

**Artifact Type:** `Card`

---

### logs cleanup

Clean up old log entries.

```bash
sp infra logs cleanup --older-than 30d --dry-run
sp infra logs cleanup --keep-last-days 7 --dry-run
sp infra logs cleanup --older-than 30d --yes
sp infra logs cleanup --keep-last-days 7 --yes
```

**Required Flags (one of):**
| Flag | Description |
|------|-------------|
| `--older-than` | Delete logs older than duration (e.g., `7d`, `24h`, `30d`) |
| `--keep-last-days` | Keep logs from the last N days |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--dry-run` | `false` | Preview without deleting |
| `--yes`, `-y` | `false` | Skip confirmation (required in non-interactive mode) |

**Output Structure:**
```json
{
  "deleted_count": 5000,
  "dry_run": false,
  "cutoff_date": "2023-12-15T00:00:00Z",
  "vacuum_performed": false
}
```

**Artifact Type:** `Card`

---

### logs delete

Delete all log entries.

```bash
sp infra logs delete --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--yes` / `-y` | Yes | Confirm deletion |

**Output Structure:**
```json
{
  "deleted_count": 15000,
  "vacuum_performed": false
}
```

**Artifact Type:** `Card`

---

### logs summary

Show aggregate statistics about logs.

```bash
sp infra logs summary
sp --json logs summary
sp infra logs summary --since 24h
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--since` | None | Only include logs since this duration |

**Output Structure:**
```json
{
  "total_logs": 5000,
  "by_level": {
    "error": 12,
    "warn": 45,
    "info": 4900,
    "debug": 43,
    "trace": 0
  },
  "top_modules": [
    {"module": "agent::handler", "count": 1500},
    {"module": "database::pool", "count": 800}
  ],
  "time_range": {
    "earliest": "2024-01-01 00:00:00",
    "latest": "2024-01-15 23:59:59",
    "span_hours": 360
  },
  "database_info": {
    "logs_table_rows": 5000
  }
}
```

**Artifact Type:** `Card`

---

## Trace Commands

### logs trace list

List execution traces for debugging.

```bash
sp infra logs trace list
sp --json logs trace list
sp infra logs trace list -n 50
sp infra logs trace list --since 1h
sp infra logs trace list --agent primary
sp infra logs trace list --status completed
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit`, `-n` | `20` | Maximum results |
| `--since` | None | Time filter |
| `--agent` | None | Filter by agent name |
| `--status` | None | Filter by status (completed, failed, running) |

**Output Structure:**
```json
{
  "traces": [
    {
      "trace_id": "trace_abc123",
      "timestamp": "2024-01-15 10:30:00",
      "agent": "primary",
      "status": "completed",
      "duration_ms": 1250,
      "ai_requests": 3,
      "mcp_calls": 5
    }
  ],
  "total": 20
}
```

**Artifact Type:** `Table`
**Columns:** `trace_id`, `timestamp`, `agent`, `status`, `duration_ms`, `ai_requests`, `mcp_calls`

---

### logs trace show

View detailed execution trace. Supports both trace IDs and task IDs.

```bash
sp infra logs trace show <trace-id>
sp infra logs trace show <task-id>
sp infra logs trace show abc123 --verbose
sp infra logs trace show abc123 --all
sp infra logs trace show abc123 --steps --ai --mcp
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Trace ID or Task ID (can be partial) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--verbose` | `false` | Show detailed metadata for each event |
| `--json` | `false` | Output as JSON |
| `--steps` | `false` | Show execution steps |
| `--ai` | `false` | Show AI requests in trace |
| `--mcp` | `false` | Show MCP tool calls in trace |
| `--artifacts` | `false` | Show artifacts |
| `--all` | `false` | Show all sections (steps, ai, mcp, artifacts) |

**Output Structure:**
```json
{
  "trace_id": "trace_abc123",
  "events": [...],
  "ai_summary": {
    "request_count": 2,
    "total_tokens": 1500,
    "input_tokens": 1200,
    "output_tokens": 300,
    "cost_dollars": 0.015,
    "total_latency_ms": 2500
  },
  "mcp_summary": {
    "execution_count": 3,
    "total_execution_time_ms": 450
  },
  "step_summary": {
    "total": 5,
    "completed": 4,
    "failed": 0,
    "pending": 1
  },
  "task_id": "task_xyz789",
  "duration_ms": 3000,
  "status": "completed"
}
```

**Artifact Type:** `Card`

---

## Request Commands

### logs request list

List AI requests.

```bash
sp infra logs request list
sp --json logs request list
sp infra logs request list -n 50
sp infra logs request list --since 1h
sp infra logs request list --provider anthropic
sp infra logs request list --model claude-3-5-sonnet-20241022
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit`, `-n` | `20` | Maximum results |
| `--since` | None | Time filter |
| `--provider` | None | Filter by provider |
| `--model` | None | Filter by model |

**Output Structure:**
```json
{
  "requests": [
    {
      "request_id": "req_abc123",
      "timestamp": "2024-01-15 10:30:00",
      "provider": "anthropic",
      "model": "claude-3-5-sonnet-20241022",
      "tokens": "500/200",
      "cost": "$0.000700",
      "latency_ms": 850
    }
  ],
  "total": 20
}
```

**Artifact Type:** `Table`
**Columns:** `request_id`, `timestamp`, `provider`, `model`, `tokens`, `cost`, `latency_ms`

---

### logs request show

Show detailed AI request.

```bash
sp infra logs request show <request-id>
sp --json logs request show req_abc123
sp infra logs request show abc123 --messages
sp infra logs request show abc123 --tools
sp infra logs request show abc123 --messages --tools
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Request ID (can be partial) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--messages`, `-m` | `false` | Show conversation messages |
| `--tools`, `-t` | `false` | Show linked MCP tool calls |

**Output Structure:**
```json
{
  "request_id": "req_abc123",
  "provider": "anthropic",
  "model": "claude-3-5-sonnet-20241022",
  "input_tokens": 500,
  "output_tokens": 200,
  "cost_dollars": 0.0007,
  "latency_ms": 850,
  "messages": [
    {"sequence": 0, "role": "system", "content": "You are..."},
    {"sequence": 1, "role": "user", "content": "Hello"},
    {"sequence": 2, "role": "assistant", "content": "Hi there!"}
  ],
  "linked_mcp_calls": [
    {"tool_name": "search", "server": "filesystem", "status": "success", "duration_ms": 45}
  ]
}
```

**Artifact Type:** `Card`

---

### logs request stats

Show aggregate AI request statistics.

```bash
sp infra logs request stats
sp --json logs request stats
sp infra logs request stats --since 24h
sp infra logs request stats --since 7d
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--since` | None | Only include requests since this duration |

**Output Structure:**
```json
{
  "total_requests": 150,
  "total_tokens": {
    "input": 75000,
    "output": 15000,
    "total": 90000
  },
  "total_cost_dollars": 0.45,
  "average_latency_ms": 1200,
  "by_provider": [
    {
      "provider": "anthropic",
      "request_count": 100,
      "total_tokens": 60000,
      "total_cost_dollars": 0.35,
      "avg_latency_ms": 1100
    },
    {
      "provider": "openai",
      "request_count": 50,
      "total_tokens": 30000,
      "total_cost_dollars": 0.10,
      "avg_latency_ms": 1400
    }
  ],
  "by_model": [
    {
      "model": "claude-3-5-sonnet-20241022",
      "provider": "anthropic",
      "request_count": 80,
      "total_tokens": 50000,
      "total_cost_dollars": 0.30,
      "avg_latency_ms": 1050
    }
  ]
}
```

**Artifact Type:** `Card`

---

## Tracing Agent Messages

When you send a message to an agent via the A2A protocol, you can trace the full execution flow using the logs commands.

### Step 1: Send Message and Get Task ID

```bash
# Send a message and capture the response
RESPONSE=$(sp --json agents message admin -m "What is 2+2?" --token "$TOKEN" --blocking)
echo "$RESPONSE"

# Extract task_id and context_id from response
TASK_ID=$(echo "$RESPONSE" | jq -r '.data.task.task_id')
CONTEXT_ID=$(echo "$RESPONSE" | jq -r '.data.task.context_id')
echo "Task ID: $TASK_ID"
```

### Step 2: View the Trace

The `infra logs trace show` command accepts both trace IDs and task IDs:

```bash
# View trace by task ID (automatically resolves to trace)
sp infra logs trace show "$TASK_ID" --all

# Or list recent traces and find your trace
sp infra logs trace list --since 5m
```

### Step 3: Inspect AI Requests

```bash
# List recent AI requests
sp infra logs request list --since 5m

# Show details of a specific request including the full conversation
sp infra logs request show <request-id> --messages --tools
```

### Step 4: Get Aggregate Statistics

```bash
# Summary of all logs
sp infra logs summary --since 1h

# AI request statistics
sp infra logs request stats --since 1h
```

### Complete Tracing Flow Example

```bash
# Phase 1: Send message to agent
TOKEN=$(sp infra system login --email admin@example.com --token-only)
RESPONSE=$(sp --json agents message admin -m "Show me traffic stats" --token "$TOKEN" --blocking)
TASK_ID=$(echo "$RESPONSE" | jq -r '.data.task.task_id')

# Phase 2: View the execution trace
sp infra logs trace show "$TASK_ID" --all

# Phase 3: View specific AI requests made during the task
sp infra logs request list --since 5m
sp infra logs request show <request-id> --messages

# Phase 4: Check aggregate statistics
sp infra logs request stats --since 1h
```

**Related Documentation:** See [agents/README.md](../agents/README.md) for details on sending messages to agents.

---

## Complete Logs Management Flow Example

```bash
# Phase 1: View recent logs
sp --json logs view --tail 20

# Phase 2: Get summary statistics
sp --json logs summary --since 24h

# Phase 3: Search for errors
sp --json logs search "error" --since 24h

# Phase 4: View specific module logs
sp --json logs view --module agent --level warn

# Phase 5: Export logs for analysis
sp infra logs export --format json --since 7d -o ./weekly-logs.json

# Phase 6: Trace debugging
sp --json logs trace list --since 1h
sp infra logs trace show trace_abc123 --all

# Phase 7: AI request analysis
sp --json logs request list --since 24h --provider anthropic
sp --json logs request show req_abc123 --messages --tools
sp --json logs request stats --since 24h

# Phase 8: Cleanup old logs
sp infra logs cleanup --older-than 30d --dry-run
sp infra logs cleanup --older-than 30d --yes
```

---

## Log Level Reference

| Level | Description |
|-------|-------------|
| `debug` | Detailed debugging information |
| `info` | General informational messages |
| `warn` | Warning messages for potential issues |
| `error` | Error messages for failures |
| `trace` | Very detailed tracing information |

---

## Time Range Format

| Format | Description |
|--------|-------------|
| `1h` | 1 hour |
| `24h` | 24 hours |
| `7d` | 7 days |
| `30d` | 30 days |
| `2024-01-15` | Specific date |
| `2024-01-15T10:00:00` | Specific datetime |

---

## Error Handling

### Missing Required Flags

```bash
sp infra logs delete
# Error: --yes is required in non-interactive mode

sp infra logs cleanup
# Error: Either --older-than or --keep-last-days is required
```

### Trace Not Found

```bash
sp infra logs trace show nonexistent
# Warning: No events found for trace: nonexistent
# Tip: The trace may take a moment to populate. Try again in a few seconds.
```

### Request Not Found

```bash
sp infra logs request show nonexistent
# Warning: AI request not found: nonexistent
# Tip: Use 'systemprompt logs request list' to see recent requests
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json logs view | jq .

# Extract specific fields
sp --json logs view | jq '.data.logs[].message'
sp --json logs trace list | jq '.data.traces[] | {trace_id, duration_ms}'
sp --json logs request list | jq '.data.requests[] | select(.latency_ms > 1000)'
sp --json logs request stats | jq '.data.total_cost_dollars'
sp --json logs summary | jq '.data.by_level'

# Filter by criteria
sp --json logs view | jq '.data.logs[] | select(.level == "ERROR")'
sp --json logs trace list | jq '.data.traces[] | select(.status == "failed")'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` command requires `--yes` / `-y` flag
- [x] `cleanup` command requires `--older-than` or `--keep-last-days`
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Time range filters consistent across commands
- [x] `-n` shortcut available for limit flags
