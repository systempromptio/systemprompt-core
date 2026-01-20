# Analytics CLI Commands

This document provides complete documentation for AI agents to use the analytics CLI commands. All commands support non-interactive mode for automation.

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
| `analytics overview` | Dashboard overview of all analytics | `Dashboard` | No (DB only) |
| `analytics conversations stats` | Conversation statistics | `Card` | No (DB only) |
| `analytics conversations trends` | Conversation trends over time | `Table` | No (DB only) |
| `analytics conversations list` | List conversations | `Table` | No (DB only) |
| `analytics agents stats` | Aggregate agent statistics | `Card` | No (DB only) |
| `analytics agents list` | List agents with metrics | `Table` | No (DB only) |
| `analytics agents trends` | Agent usage trends | `Table` | No (DB only) |
| `analytics agents show <name>` | Deep dive into specific agent | `Card` | No (DB only) |
| `analytics tools stats` | Aggregate tool statistics | `Card` | No (DB only) |
| `analytics tools list` | List tools with metrics | `Table` | No (DB only) |
| `analytics tools trends` | Tool usage trends | `Table` | No (DB only) |
| `analytics tools show <name>` | Deep dive into specific tool | `Card` | No (DB only) |
| `analytics requests stats` | AI request statistics | `Card` | No (DB only) |
| `analytics requests trends` | AI request trends | `Table` | No (DB only) |
| `analytics requests models` | Model usage breakdown | `Table` | No (DB only) |
| `analytics sessions stats` | Session statistics | `Card` | No (DB only) |
| `analytics sessions trends` | Session trends | `Table` | No (DB only) |
| `analytics sessions live` | Real-time active sessions | `Table` | No (DB only) |
| `analytics content stats` | Content engagement statistics | `Card` | No (DB only) |
| `analytics content top` | Top performing content | `Table` | No (DB only) |
| `analytics content trends` | Content trends | `Table` | No (DB only) |
| `analytics traffic sources` | Traffic source breakdown | `Table` | No (DB only) |
| `analytics traffic geo` | Geographic distribution | `Table` | No (DB only) |
| `analytics traffic devices` | Device and browser breakdown | `Table` | No (DB only) |
| `analytics traffic bots` | Bot traffic analysis | `Table` | No (DB only) |
| `analytics costs summary` | Cost summary | `Card` | No (DB only) |
| `analytics costs trends` | Cost trends over time | `Table` | No (DB only) |
| `analytics costs breakdown` | Cost breakdown by model/agent | `Table` | No (DB only) |

---

## Common Flags

All analytics commands share these common flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--since` | `24h` or `7d` | Time range start (e.g., '1h', '24h', '7d', '30d') |
| `--until` | Now | End time for range |
| `--export` | None | Export results to CSV file |

**Default Time Ranges:**
- Stats/list commands default to `24h` (recent snapshot)
- Trends commands default to `7d` (meaningful trend data needs multiple data points)

**Time Range Formats:**
- Hours: `1h`, `2h`, `24h`
- Days: `1d`, `7d`, `30d`
- ISO datetime: `2024-01-15T00:00:00Z`

---

## Overview Command

### analytics overview

Dashboard overview of all analytics metrics in one view.

```bash
sp analytics overview
sp --json analytics overview
sp analytics overview --since 7d
sp analytics overview --since 24h --export metrics.csv
```

**Output Structure:**
```json
{
  "period": "2024-01-14 00:00 to 2024-01-15 00:00",
  "conversations": {
    "total": 150,
    "change_percent": 12.5
  },
  "agents": {
    "active_count": 3,
    "total_tasks": 450,
    "success_rate": 95.2
  },
  "requests": {
    "total": 1200,
    "total_tokens": 500000,
    "avg_latency_ms": 850
  },
  "tools": {
    "total_executions": 320,
    "success_rate": 98.5
  },
  "sessions": {
    "active": 15,
    "total_today": 89
  },
  "costs": {
    "total_cents": 4520,
    "change_percent": -5.2
  }
}
```

**Artifact Type:** `Dashboard`

---

## Conversations Commands

### analytics conversations stats

Aggregate conversation statistics.

```bash
sp analytics conversations stats
sp --json analytics conversations stats
sp analytics conversations stats --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_contexts": 150,
  "total_tasks": 450,
  "total_messages": 2300,
  "avg_messages_per_task": 5.1,
  "avg_task_duration_ms": 12500
}
```

**Artifact Type:** `Card`

---

### analytics conversations trends

Conversation trends over time.

```bash
sp analytics conversations trends
sp --json analytics conversations trends
sp analytics conversations trends --since 7d --group-by day
sp analytics conversations trends --group-by hour
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "context_count": 45,
      "task_count": 120,
      "message_count": 580
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `context_count`, `task_count`, `message_count`

---

### analytics conversations list

List conversations with details.

```bash
sp analytics conversations list
sp --json analytics conversations list
sp analytics conversations list --limit 20
sp analytics conversations list --since 7d
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `50` | Maximum number of results |

**Output Structure:**
```json
{
  "conversations": [
    {
      "context_id": "ctx_abc123",
      "name": "Code Review Session",
      "task_count": 5,
      "message_count": 23,
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T11:45:00Z"
    }
  ],
  "total": 150
}
```

**Artifact Type:** `Table`
**Columns:** `context_id`, `name`, `task_count`, `message_count`, `created_at`

---

## Agents Commands

### analytics agents stats

Aggregate agent statistics.

```bash
sp analytics agents stats
sp --json analytics agents stats
sp analytics agents stats --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_agents": 3,
  "total_tasks": 450,
  "completed_tasks": 425,
  "failed_tasks": 25,
  "success_rate": 94.4,
  "avg_execution_time_ms": 8500,
  "total_ai_requests": 1200,
  "total_cost_cents": 4520
}
```

**Artifact Type:** `Card`

---

### analytics agents list

List agents with performance metrics.

```bash
sp analytics agents list
sp --json analytics agents list
sp analytics agents list --since 7d --limit 10
sp analytics agents list --sort-by success-rate
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--sort-by` | `task-count` | Sort by: `task-count`, `success-rate`, `cost`, `last-active` |

**Output Structure:**
```json
{
  "agents": [
    {
      "agent_name": "primary",
      "task_count": 200,
      "success_rate": 96.5,
      "avg_execution_time_ms": 7500,
      "total_cost_cents": 2100,
      "last_active": "2024-01-15T11:30:00Z"
    }
  ],
  "total": 3
}
```

**Artifact Type:** `Table`
**Columns:** `agent_name`, `task_count`, `success_rate`, `avg_execution_time_ms`, `total_cost_cents`

---

### analytics agents trends

Agent usage trends over time.

```bash
sp analytics agents trends
sp --json analytics agents trends
sp analytics agents trends --agent primary
sp analytics agents trends --since 7d --group-by day
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--agent` | All | Filter by specific agent name |
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "agent": "primary",
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "task_count": 45,
      "success_rate": 95.6,
      "avg_execution_time_ms": 7800
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `task_count`, `success_rate`, `avg_execution_time_ms`

---

### analytics agents show

Deep dive into a specific agent's performance.

```bash
sp analytics agents show <agent-name>
sp --json analytics agents show primary
sp analytics agents show primary --since 7d
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Agent name to analyze |

**Output Structure:**
```json
{
  "agent_name": "primary",
  "period": "2024-01-14 to 2024-01-15",
  "summary": {
    "total_tasks": 200,
    "completed_tasks": 192,
    "failed_tasks": 8,
    "success_rate": 96.0
  },
  "status_breakdown": [
    {"status": "completed", "count": 192, "percentage": 96.0},
    {"status": "failed", "count": 8, "percentage": 4.0}
  ],
  "top_errors": [
    {"error_type": "timeout", "count": 5},
    {"error_type": "rate_limit", "count": 3}
  ],
  "hourly_distribution": [
    {"hour": 9, "count": 25},
    {"hour": 10, "count": 32}
  ]
}
```

**Artifact Type:** `Card`

---

## Tools Commands

### analytics tools stats

Aggregate MCP tool statistics.

```bash
sp analytics tools stats
sp --json analytics tools stats
sp analytics tools stats --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_tools": 15,
  "total_executions": 320,
  "successful": 310,
  "failed": 8,
  "timeout": 2,
  "success_rate": 96.9,
  "avg_execution_time_ms": 450,
  "p95_execution_time_ms": 1200
}
```

**Artifact Type:** `Card`

---

### analytics tools list

List tools with execution metrics.

```bash
sp analytics tools list
sp --json analytics tools list
sp analytics tools list --since 7d --server systemprompt-admin
sp analytics tools list --sort-by success-rate
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--server` | All | Filter by server name |
| `--sort-by` | `execution-count` | Sort by: `execution-count`, `success-rate`, `avg-time` |

**Output Structure:**
```json
{
  "tools": [
    {
      "tool_name": "read_file",
      "server_name": "filesystem",
      "execution_count": 150,
      "success_rate": 99.3,
      "avg_execution_time_ms": 120,
      "last_used": "2024-01-15T11:30:00Z"
    }
  ],
  "total": 15
}
```

**Artifact Type:** `Table`
**Columns:** `tool_name`, `server_name`, `execution_count`, `success_rate`, `avg_execution_time_ms`

---

### analytics tools trends

Tool usage trends over time.

```bash
sp analytics tools trends
sp --json analytics tools trends
sp analytics tools trends --tool read_file
sp analytics tools trends --since 7d --group-by day
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--tool` | All | Filter by specific tool name |
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "tool": "read_file",
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "execution_count": 45,
      "success_rate": 98.9,
      "avg_execution_time_ms": 115
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `execution_count`, `success_rate`, `avg_execution_time_ms`

---

### analytics tools show

Deep dive into a specific tool's performance.

```bash
sp analytics tools show <tool-name>
sp --json analytics tools show read_file
sp analytics tools show read_file --since 7d
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Tool name to analyze |

**Output Structure:**
```json
{
  "tool_name": "read_file",
  "period": "2024-01-14 to 2024-01-15",
  "summary": {
    "total_executions": 150,
    "success_rate": 99.3
  },
  "status_breakdown": [
    {"status": "success", "count": 149, "percentage": 99.3},
    {"status": "error", "count": 1, "percentage": 0.7}
  ],
  "top_errors": [
    {"error_message": "File not found", "count": 1}
  ],
  "usage_by_agent": [
    {"agent_name": "primary", "count": 120, "percentage": 80.0},
    {"agent_name": "secondary", "count": 30, "percentage": 20.0}
  ]
}
```

**Artifact Type:** `Card`

---

## Requests Commands

### analytics requests stats

Aggregate AI request statistics.

```bash
sp analytics requests stats
sp --json analytics requests stats
sp analytics requests stats --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_requests": 1200,
  "total_tokens": 500000,
  "input_tokens": 350000,
  "output_tokens": 150000,
  "total_cost_cents": 4520,
  "avg_latency_ms": 850,
  "cache_hit_rate": 35.2
}
```

**Artifact Type:** `Card`

---

### analytics requests trends

AI request trends over time.

```bash
sp analytics requests trends
sp --json analytics requests trends
sp analytics requests trends --since 7d --group-by day
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "request_count": 180,
      "total_tokens": 75000,
      "cost_cents": 680,
      "avg_latency_ms": 820
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `request_count`, `total_tokens`, `cost_cents`, `avg_latency_ms`

---

### analytics requests models

Model usage breakdown.

```bash
sp analytics requests models
sp --json analytics requests models
sp analytics requests models --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "models": [
    {
      "provider": "anthropic",
      "model": "claude-3-5-sonnet-20241022",
      "request_count": 800,
      "total_tokens": 350000,
      "total_cost_cents": 3150,
      "avg_latency_ms": 920,
      "percentage": 66.7
    },
    {
      "provider": "openai",
      "model": "gpt-4-turbo",
      "request_count": 400,
      "total_tokens": 150000,
      "total_cost_cents": 1370,
      "avg_latency_ms": 750,
      "percentage": 33.3
    }
  ],
  "total_requests": 1200
}
```

**Artifact Type:** `Table`
**Columns:** `provider`, `model`, `request_count`, `total_tokens`, `total_cost_cents`, `percentage`

---

## Sessions Commands

### analytics sessions stats

Session statistics.

```bash
sp analytics sessions stats
sp --json analytics sessions stats
sp analytics sessions stats --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_sessions": 89,
  "active_sessions": 15,
  "unique_users": 45,
  "avg_duration_seconds": 1850,
  "avg_requests_per_session": 12.5,
  "conversion_rate": 8.5
}
```

**Artifact Type:** `Card`

---

### analytics sessions trends

Session trends over time.

```bash
sp analytics sessions trends
sp --json analytics sessions trends
sp analytics sessions trends --since 7d --group-by day
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "session_count": 12,
      "active_users": 8,
      "avg_duration_seconds": 1920
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `session_count`, `active_users`, `avg_duration_seconds`

---

### analytics sessions live

Real-time active sessions monitor.

```bash
sp analytics sessions live
sp --json analytics sessions live
sp analytics sessions live --limit 20
sp analytics sessions live --no-refresh
sp analytics sessions live --export sessions.csv
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum sessions to show |
| `--refresh` | `5` | Refresh interval in seconds |
| `--no-refresh` | - | Show once without auto-refresh |
| `--export` | None | Export to CSV (single snapshot) |

**Output Structure:**
```json
{
  "active_count": 15,
  "sessions": [
    {
      "session_id": "sess_abc123",
      "user_type": "authenticated",
      "started_at": "2024-01-15T10:30:00Z",
      "duration_seconds": 1850,
      "request_count": 25,
      "last_activity": "2024-01-15T11:01:00Z"
    }
  ],
  "timestamp": "2024-01-15T11:05:00Z"
}
```

**Artifact Type:** `Table`
**Columns:** `session_id`, `user_type`, `duration_seconds`, `request_count`, `last_activity`

---

## Content Commands

### analytics content stats

Content engagement statistics.

```bash
sp analytics content stats
sp --json analytics content stats
sp analytics content stats --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_views": 5200,
  "unique_visitors": 1850,
  "avg_time_on_page_seconds": 145,
  "avg_scroll_depth": 72.5,
  "total_clicks": 320
}
```

**Artifact Type:** `Card`

---

### analytics content top

Top performing content.

```bash
sp analytics content top
sp --json analytics content top
sp analytics content top --since 7d --limit 10
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "content": [
    {
      "content_id": "blog/getting-started",
      "views": 520,
      "unique_visitors": 380,
      "avg_time_seconds": 185,
      "trend": "up"
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `content_id`, `views`, `unique_visitors`, `avg_time_seconds`, `trend`

---

### analytics content trends

Content engagement trends over time.

```bash
sp analytics content trends
sp --json analytics content trends
sp analytics content trends --since 7d --group-by day
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "views": 780,
      "unique_visitors": 285
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `views`, `unique_visitors`

---

## Traffic Commands

### analytics traffic sources

Traffic source breakdown.

```bash
sp analytics traffic sources
sp --json analytics traffic sources
sp analytics traffic sources --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "sources": [
    {"source": "direct", "session_count": 450, "percentage": 50.6},
    {"source": "organic_search", "session_count": 280, "percentage": 31.5},
    {"source": "referral", "session_count": 120, "percentage": 13.5},
    {"source": "social", "session_count": 40, "percentage": 4.4}
  ],
  "total_sessions": 890
}
```

**Artifact Type:** `Table`
**Columns:** `source`, `session_count`, `percentage`

---

### analytics traffic geo

Geographic distribution.

```bash
sp analytics traffic geo
sp --json analytics traffic geo
sp analytics traffic geo --since 7d --limit 10
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of countries |

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "countries": [
    {"country": "United States", "session_count": 420, "percentage": 47.2},
    {"country": "United Kingdom", "session_count": 150, "percentage": 16.9},
    {"country": "Germany", "session_count": 85, "percentage": 9.6}
  ],
  "total_sessions": 890
}
```

**Artifact Type:** `Table`
**Columns:** `country`, `session_count`, `percentage`

---

### analytics traffic devices

Device and browser breakdown.

```bash
sp analytics traffic devices
sp --json analytics traffic devices
sp analytics traffic devices --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "devices": [
    {"device_type": "desktop", "browser": "Chrome", "session_count": 520, "percentage": 58.4},
    {"device_type": "desktop", "browser": "Firefox", "session_count": 180, "percentage": 20.2},
    {"device_type": "mobile", "browser": "Safari", "session_count": 120, "percentage": 13.5}
  ],
  "total_sessions": 890
}
```

**Artifact Type:** `Table`
**Columns:** `device_type`, `browser`, `session_count`, `percentage`

---

### analytics traffic bots

Bot traffic analysis.

```bash
sp analytics traffic bots
sp --json analytics traffic bots
sp analytics traffic bots --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "human_sessions": 890,
  "bot_sessions": 2150,
  "bot_percentage": 70.7,
  "bot_breakdown": [
    {"bot_type": "googlebot", "request_count": 1200, "percentage": 55.8},
    {"bot_type": "bingbot", "request_count": 450, "percentage": 20.9},
    {"bot_type": "other", "request_count": 500, "percentage": 23.3}
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `bot_type`, `request_count`, `percentage`

---

## Costs Commands

### analytics costs summary

Cost summary.

```bash
sp analytics costs summary
sp --json analytics costs summary
sp analytics costs summary --since 7d
```

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "total_cost_cents": 4520,
  "total_requests": 1200,
  "total_tokens": 500000,
  "avg_cost_per_request_cents": 3.77,
  "change_percent": -5.2
}
```

**Artifact Type:** `Card`

---

### analytics costs trends

Cost trends over time.

```bash
sp analytics costs trends
sp --json analytics costs trends
sp analytics costs trends --since 7d --group-by day
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--group-by` | `hour` | Grouping: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "period": "2024-01-08 to 2024-01-15",
  "group_by": "day",
  "points": [
    {
      "timestamp": "2024-01-14",
      "cost_cents": 680,
      "request_count": 180,
      "tokens": 75000
    }
  ],
  "total_cost_cents": 4520
}
```

**Artifact Type:** `Table`
**Columns:** `timestamp`, `cost_cents`, `request_count`, `tokens`

---

### analytics costs breakdown

Cost breakdown by model or agent.

```bash
sp analytics costs breakdown
sp --json analytics costs breakdown
sp analytics costs breakdown --by model
sp analytics costs breakdown --by agent
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--by` | `model` | Breakdown by: `model`, `agent` |

**Output Structure:**
```json
{
  "period": "2024-01-14 to 2024-01-15",
  "breakdown_by": "model",
  "items": [
    {
      "name": "claude-3-5-sonnet-20241022",
      "cost_cents": 3150,
      "request_count": 800,
      "tokens": 350000,
      "percentage": 69.7
    },
    {
      "name": "gpt-4-turbo",
      "cost_cents": 1370,
      "request_count": 400,
      "tokens": 150000,
      "percentage": 30.3
    }
  ],
  "total_cost_cents": 4520
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `cost_cents`, `request_count`, `tokens`, `percentage`

---

## Complete Analytics Flow Example

This flow demonstrates a comprehensive analytics review:

```bash
# Phase 1: Get overview dashboard
sp --json analytics overview --since 24h

# Phase 2: Drill into conversations
sp --json analytics conversations stats --since 24h
sp --json analytics conversations trends --since 7d --group-by day

# Phase 3: Analyze agent performance
sp --json analytics agents stats --since 24h
sp --json analytics agents list --since 7d
sp --json analytics agents show primary --since 7d

# Phase 4: Review tool usage
sp --json analytics tools stats --since 24h
sp --json analytics tools list --sort-by success-rate
sp --json analytics tools show read_file

# Phase 5: Examine AI requests
sp --json analytics requests stats --since 24h
sp --json analytics requests models --since 7d
sp --json analytics requests trends --since 7d --group-by day

# Phase 6: Check sessions
sp --json analytics sessions stats --since 24h
sp --json analytics sessions live

# Phase 7: Review content performance
sp --json analytics content stats --since 24h
sp --json analytics content top --limit 10

# Phase 8: Analyze traffic
sp --json analytics traffic sources --since 7d
sp --json analytics traffic geo --limit 10
sp --json analytics traffic bots

# Phase 9: Review costs
sp --json analytics costs summary --since 30d
sp --json analytics costs breakdown --by model
sp --json analytics costs trends --since 30d --group-by day

# Phase 10: Export comprehensive report
sp analytics overview --since 7d --export weekly-overview.csv
sp analytics costs trends --since 30d --export monthly-costs.csv
```

---

## Output Type Summary

| Command | Return Type | Artifact Type | Metadata |
|---------|-------------|---------------|----------|
| `overview` | `OverviewOutput` | `Dashboard` | title |
| `conversations stats` | `ConversationStatsOutput` | `Card` | title |
| `conversations trends` | `ConversationTrendsOutput` | `Table` | columns |
| `conversations list` | `ConversationListOutput` | `Table` | columns |
| `admin agents stats` | `AgentStatsOutput` | `Card` | title |
| `admin agents list` | `AgentListOutput` | `Table` | columns |
| `admin agents trends` | `AgentTrendsOutput` | `Table` | columns |
| `admin agents show` | `AgentShowOutput` | `Card` | title |
| `tools stats` | `ToolStatsOutput` | `Card` | title |
| `tools list` | `ToolListOutput` | `Table` | columns |
| `tools trends` | `ToolTrendsOutput` | `Table` | columns |
| `tools show` | `ToolShowOutput` | `Card` | title |
| `requests stats` | `RequestStatsOutput` | `Card` | title |
| `requests trends` | `RequestTrendsOutput` | `Table` | columns |
| `requests models` | `ModelsOutput` | `Table` | columns |
| `sessions stats` | `SessionStatsOutput` | `Card` | title |
| `sessions trends` | `SessionTrendsOutput` | `Table` | columns |
| `sessions live` | `LiveSessionsOutput` | `Table` | columns |
| `core content stats` | `ContentStatsOutput` | `Card` | title |
| `core content top` | `TopContentOutput` | `Table` | columns |
| `core content trends` | `ContentTrendsOutput` | `Table` | columns |
| `traffic sources` | `TrafficSourcesOutput` | `Table` | columns |
| `traffic geo` | `GeoOutput` | `Table` | columns |
| `traffic devices` | `DevicesOutput` | `Table` | columns |
| `traffic bots` | `BotsOutput` | `Table` | columns |
| `costs summary` | `CostSummaryOutput` | `Card` | title |
| `costs trends` | `CostTrendsOutput` | `Table` | columns |
| `costs breakdown` | `CostBreakdownOutput` | `Table` | columns |

---

## Error Handling

### No Data Errors

```bash
sp analytics agents show nonexistent
# Error: Agent 'nonexistent' not found in analytics data

sp analytics tools show nonexistent
# Error: Tool 'nonexistent' not found in analytics data
```

### Database Connection Errors

```bash
sp analytics overview
# Error: Failed to connect to database. Check your profile configuration.
```

### Invalid Time Range

```bash
sp analytics overview --since invalid
# Error: Invalid time range format. Use '1h', '24h', '7d', or ISO datetime.
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json analytics overview | jq .

# Extract specific metrics
sp --json analytics overview | jq '.costs.total_cents'
sp --json analytics agents list | jq '.agents[].agent_name'
sp --json analytics requests models | jq '.models[] | select(.percentage > 50)'
sp --json analytics sessions live | jq '.sessions | length'
sp --json analytics costs breakdown | jq '.items | sort_by(.cost_cents) | reverse'

# Filter by criteria
sp --json analytics agents list | jq '.agents[] | select(.success_rate < 95)'
sp --json analytics tools list | jq '.tools[] | select(.success_rate < 90)'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] CSV export supported via `--export` flag
- [x] Common time range flags (`--since`, `--until`) across all commands
