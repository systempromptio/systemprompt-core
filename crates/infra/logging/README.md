# systemprompt-core-logging

Core logging module for SystemPrompt.

## Structure

```
src/
├── lib.rs                          - Module entry, init_logging(), init_console_logging()
│
├── layer/
│   ├── mod.rs                      - DatabaseLayer for async log persistence
│   └── visitor.rs                  - FieldVisitor, SpanVisitor, field_names constants
│
├── models/
│   ├── mod.rs                      - Module exports
│   ├── log_entry.rs                - LogEntry with builder pattern
│   ├── log_error.rs                - LoggingError enum
│   ├── log_filter.rs               - LogFilter for paginated queries
│   ├── log_level.rs                - LogLevel enum
│   └── log_row.rs                  - LogRow for SQLx mapping
│
├── repository/
│   ├── mod.rs                      - LoggingRepository CRUD operations
│   ├── analytics/
│   │   └── mod.rs                  - AnalyticsRepository, AnalyticsEvent
│   └── operations/
│       ├── mod.rs                  - Operation exports
│       ├── queries.rs              - get_log, list_logs, list_logs_paginated
│       └── mutations.rs            - create_log, update_log, delete_log, cleanup
│
└── services/
    ├── mod.rs                      - Service exports
    ├── database_log.rs             - DatabaseLogService implementing LogService trait
    │
    ├── output/
    │   └── mod.rs                  - Startup mode control, log publisher
    │
    ├── spans/
    │   └── mod.rs                  - RequestSpan, SystemSpan, RequestSpanBuilder
    │
    ├── cli/
    │   ├── mod.rs                  - CliService facade
    │   ├── display.rs              - Display traits, render_table, truncate_to_width
    │   ├── module.rs               - ModuleDisplay, ModuleInstall, ModuleUpdate
    │   ├── prompts.rs              - Prompts, PromptBuilder, QuickPrompts
    │   ├── summary.rs              - ValidationSummary, OperationResult, ProgressSummary
    │   └── theme.rs                - Icons, Colors, Theme, MessageLevel, ItemStatus
    │
    └── retention/
        ├── mod.rs                  - Module exports
        ├── policies.rs             - RetentionPolicy, RetentionConfig
        └── scheduler.rs            - RetentionScheduler for cron-based cleanup
```

## Public API

- `init_logging(db_pool)` - Initialize logging with database persistence
- `init_console_logging()` - Initialize console-only logging
- `DatabaseLayer` - Tracing layer for async log persistence
- `LogEntry`, `LogLevel`, `LogFilter`, `LoggingError` - Core types
- `LoggingRepository` - Database operations for logs
- `DatabaseLogService` - Trait-based service implementing `LogService` from systemprompt-traits
- `AnalyticsRepository`, `AnalyticsEvent` - Analytics tracking
- `RequestSpan`, `SystemSpan`, `RequestSpanBuilder` - Tracing context wrappers
- `CliService` - CLI output facade
- `is_startup_mode()`, `set_startup_mode()` - Startup mode control
- `RetentionConfig`, `RetentionPolicy`, `RetentionScheduler` - Log lifecycle management
