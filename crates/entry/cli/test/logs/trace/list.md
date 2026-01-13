# logs trace list

## Status
**PASS**

## Command
```
systemprompt --non-interactive logs trace list --limit 5
```

## Output
```
Recent Traces
{
  "traces": [
    {
      "trace_id": "523426fa-7673-484c-b605-28cf0aec48b6",
      "timestamp": "2026-01-13 21:30:03",
      "status": "unknown",
      "duration_ms": 24,
      "ai_requests": 0,
      "mcp_calls": 0
    },
    {
      "trace_id": "trace_53504697-28b2-402e-8470-ea2460005cd8",
      "timestamp": "2026-01-13 21:23:43",
      "status": "unknown",
      "duration_ms": 1,
      "ai_requests": 0,
      "mcp_calls": 0
    }
  ],
  "total": 5
}
```

## Notes
- Converted to compile-time SQL (`sqlx::query_as!`) for type safety
- Query now uses proper column aliasing with `"column!"` syntax
- Fixed table reference from `execution_tasks` to `agent_tasks`
