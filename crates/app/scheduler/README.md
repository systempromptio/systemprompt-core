# systemprompt-core-scheduler

Background job scheduling and execution module. Discovers jobs via `inventory` crate and executes them on cron schedules.

## Structure

```
src/
├── lib.rs                    # Public exports
├── jobs/
│   ├── mod.rs                # Job exports
│   ├── cleanup_empty_contexts.rs     # Removes empty conversation contexts
│   ├── cleanup_inactive_sessions.rs  # Closes inactive sessions
│   └── database_cleanup.rs           # Orphaned logs, MCP, OAuth cleanup
├── models/
│   └── mod.rs                # JobStatus, SchedulerError, ScheduledJob
├── repository/
│   ├── mod.rs                # SchedulerRepository facade
│   ├── analytics/
│   │   └── mod.rs            # Analytics queries
│   ├── images/
│   │   └── mod.rs            # Image optimization tracking
│   └── jobs/
│       └── mod.rs            # Scheduled job CRUD
└── services/
    ├── mod.rs                # Service exports
    └── scheduling/
        └── mod.rs            # SchedulerService - job discovery and execution
```

## Infrastructure Cleanup Jobs

| Job | Schedule | Description |
|-----|----------|-------------|
| `cleanup_inactive_sessions` | Every 10 min | Closes sessions inactive for 1 hour |
| `cleanup_empty_contexts` | Every 2 hours | Removes conversation contexts with no messages |
| `database_cleanup` | Daily at 3 AM | Deletes orphaned logs, MCP executions, expired OAuth tokens |

## Job Discovery

Jobs are discovered via the `inventory` crate. Domain modules register their jobs:

```rust
inventory::submit! { &ContentIngestionJob }       // in blog module
```

The scheduler discovers and runs all registered jobs based on configuration.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-core-database` | Database pool |
| `systemprompt-core-logging` | SystemSpan for tracing |
| `systemprompt-core-system` | AppContext, CleanupRepository, SessionRepository |
| `systemprompt-traits` | Job trait definition |
| `systemprompt-identifiers` | ScheduledJobId |
| `systemprompt-models` | Config types |
