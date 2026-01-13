# Logging CLI Improvement Plan

## Current Structure

```
logs
├── stream
│   ├── view      # View log entries
│   ├── delete    # Delete all logs
│   └── cleanup   # Clean old logs
└── trace
    ├── list      # List recent traces
    ├── view      # View trace by ID
    ├── ai        # View AI task details
    └── lookup    # Lookup AI request
```

## Problems

1. **Confusing naming**: `logs stream view` for viewing logs? "stream" implies real-time but it's also for one-shot viewing
2. **Redundant commands**: `trace view` vs `trace ai` - both show trace details with overlap
3. **Missing search**: No way to search logs by content/message
4. **No filtering by time**: Can't easily get logs from "last hour" or "today"
5. **Trace/AI split is unclear**: When do I use `trace ai` vs `trace lookup`?

## Proposed Structure

```
logs
├── view [--tail N] [--level LEVEL] [--module MODULE] [--since DURATION]
├── search <PATTERN> [--level LEVEL] [--since DURATION]
├── stream [--level LEVEL] [--module MODULE]  # Real-time only
├── cleanup [--older-than DURATION | --keep-last-days N]
├── delete --yes
│
├── trace
│   ├── list [--limit N] [--since DURATION] [--agent NAME] [--status STATUS]
│   └── show <TRACE_ID> [--verbose] [--json]
│
└── request
    ├── list [--limit N] [--since DURATION] [--model MODEL]
    └── show <REQUEST_ID> [--messages] [--tools]
```

## Key Changes

### 1. Flatten stream commands
- `logs view` - Primary way to view logs (was `logs stream view`)
- `logs stream` - Only for real-time streaming (removes confusion)
- `logs search` - New command for text search

### 2. Simplify trace
- Merge `trace view` and `trace ai` into `trace show`
- Keep `trace list` as is

### 3. Rename lookup to request
- `logs request list` - List AI requests (new)
- `logs request show` - Show request details (was `trace lookup`)
- Clearer separation: traces are execution flows, requests are AI calls

### 4. Add search capability
```bash
logs search "error" --since 1h --level error
logs search "agent_name" --module systemprompt_agent
```

### 5. Consistent time filtering
All commands support `--since` with duration format:
- `1h`, `24h`, `7d`, `30d`
- `2026-01-13` (date)
- `2026-01-13T10:00:00` (datetime)

## New Commands Detail

### `logs view`
```bash
logs view                           # Last 20 logs
logs view -n 100                    # Last 100 logs
logs view --since 1h                # Logs from last hour
logs view --level error             # Only errors
logs view --module agent            # Filter by module
logs view --since 1h --level error  # Combined filters
```

### `logs search`
```bash
logs search "connection failed"     # Search message content
logs search "user_123" --since 24h  # Search with time filter
logs search "timeout" --level error # Search errors only
```

### `logs trace show`
```bash
logs trace show abc123              # Show trace summary
logs trace show abc123 --verbose    # Include all events
logs trace show abc123 --json       # JSON output
logs trace show abc123 --steps      # Show execution steps
logs trace show abc123 --ai         # Show AI requests in trace
logs trace show abc123 --mcp        # Show MCP calls in trace
```

### `logs request list`
```bash
logs request list                   # Recent AI requests
logs request list --model gpt-4     # Filter by model
logs request list --since 1h        # Time filter
```

### `logs request show`
```bash
logs request show abc123            # Show request details
logs request show abc123 --messages # Include conversation
logs request show abc123 --tools    # Include tool calls
```

## Implementation Priority

1. **Phase 1**: Rename and reorganize (non-breaking aliases)
   - Add `logs view` as alias for `logs stream view`
   - Add `logs request` as alias for `trace lookup`
   - Keep old commands working with deprecation warnings

2. **Phase 2**: Add new functionality
   - Implement `logs search`
   - Add `--since` to all commands
   - Implement `logs request list`

3. **Phase 3**: Merge and simplify
   - Merge `trace view` and `trace ai` into `trace show`
   - Remove deprecated aliases

## Database Considerations

For `logs search` to be performant, consider:
- Full-text search index on `logs.message`
- Or use PostgreSQL `LIKE` with proper indexing
- Limit search to recent logs by default (e.g., last 7 days)
