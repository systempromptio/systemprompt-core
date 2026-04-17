<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-logging

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-logging — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-logging.svg?style=flat-square)](https://crates.io/crates/systemprompt-logging)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-logging?style=flat-square)](https://docs.rs/systemprompt-logging)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Tracing and audit infrastructure for systemprompt.io AI governance. Structured events, five-point audit traces, and SIEM-ready JSON output — part of the MCP governance pipeline. Provides a dual-layer logging architecture combining console output with PostgreSQL persistence, async batch processing, automatic context propagation, and retention policies.

**Layer**: Infra — infrastructure primitives (database, security, events, etc.) consumed by domain crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

This crate provides a dual-layer logging architecture combining console output with PostgreSQL persistence. It includes async batch processing, automatic context propagation, retention policies, and rich CLI output utilities.

**Key Features:**
- Dual-layer logging (console + database)
- Async batch processing with configurable flush intervals
- Automatic context propagation (user_id, session_id, task_id, trace_id)
- Cron-based log retention with per-level policies
- AI/MCP operation tracing and querying
- Rich CLI output with themes and progress indicators

## Architecture

```
src/
├── lib.rs                              - Entry point: init_logging(), init_console_logging()
│
├── layer/
│   ├── mod.rs                          - DatabaseLayer: async batch persistence to PostgreSQL
│   └── visitor.rs                      - FieldVisitor, SpanVisitor for field extraction
│
├── models/
│   ├── mod.rs                          - Re-exports
│   ├── log_entry.rs                    - LogEntry struct with builder pattern
│   ├── log_error.rs                    - LoggingError enum (thiserror)
│   ├── log_filter.rs                   - LogFilter for paginated queries
│   ├── log_level.rs                    - LogLevel enum (ERROR, WARN, INFO, DEBUG, TRACE)
│   └── log_row.rs                      - LogRow for SQLx database mapping
│
├── repository/
│   ├── mod.rs                          - LoggingRepository: CRUD with terminal/database output
│   ├── analytics/
│   │   └── mod.rs                      - AnalyticsRepository, AnalyticsEvent
│   └── operations/
│       ├── mod.rs                      - Re-exports
│       ├── queries.rs                  - get_log, list_logs, list_logs_paginated, count_logs
│       └── mutations.rs                - create_log, update_log, delete_log, cleanup_old_logs
│
├── services/
│   ├── mod.rs                          - Re-exports
│   ├── database_log.rs                 - DatabaseLogService: implements LogService trait
│   ├── format.rs                       - FilterSystemFields: filters "system" from console output
│   ├── maintenance.rs                  - LoggingMaintenanceService: cleanup operations
│   │
│   ├── cli/
│   │   ├── mod.rs                      - Module declarations and re-exports
│   │   ├── display.rs                  - Display traits, DisplayUtils, StatusDisplay, CollectionDisplay
│   │   ├── macros.rs                   - cli_success!, cli_warning!, cli_error!, cli_info! macros
│   │   ├── module.rs                   - ModuleDisplay, ModuleInstall, ModuleUpdate
│   │   ├── prompts.rs                  - Prompts, PromptBuilder, QuickPrompts
│   │   ├── service.rs                  - CliService: logging facade (success, warning, error, etc.)
│   │   ├── startup.rs                  - Startup banner and phase rendering functions
│   │   ├── summary.rs                  - ValidationSummary, OperationResult, ProgressSummary
│   │   ├── table.rs                    - Table rendering, ServiceTableEntry, render_service_table
│   │   ├── theme.rs                    - Theme, Icons, Colors, BrandColors, ServiceStatus
│   │   └── types.rs                    - ItemStatus, ModuleType, MessageLevel, IconType, ColorType
│   │
│   ├── output/
│   │   └── mod.rs                      - Startup mode: is_startup_mode(), set_startup_mode()
│   │                                     Log publisher: publish_log(), set_log_publisher()
│   │
│   ├── retention/
│   │   ├── mod.rs                      - Re-exports
│   │   ├── policies.rs                 - RetentionPolicy, RetentionConfig (per-level retention)
│   │   └── scheduler.rs                - RetentionScheduler: cron-based cleanup (daily 2AM)
│   │
│   └── spans/
│       └── mod.rs                      - RequestSpan, SystemSpan, RequestSpanBuilder
│
├── trace/
│   ├── mod.rs                          - Re-exports
│   ├── models.rs                       - TraceEvent, TaskInfo, ExecutionStep, AiRequestInfo,
│   │                                     McpToolExecution, ConversationMessage, ToolLogEntry
│   ├── service.rs                      - TraceQueryService: generic trace querying
│   ├── queries.rs                      - SQL queries for log events and AI request summaries
│   ├── step_queries.rs                 - SQL queries for MCP executions and execution steps
│   ├── ai_trace_service.rs             - AiTraceService: AI/MCP operation tracing
│   ├── ai_trace_queries.rs             - SQL queries for tasks, AI requests, messages
│   └── mcp_trace_queries.rs            - SQL queries for MCP executions, tool logs, artifacts
│
schema/
├── log.sql                             - logs table, indexes, analytical views
└── analytics.sql                       - analytics_events table with GIN indexes
```

## Usage

```toml
[dependencies]
systemprompt-logging = "0.2.1"
```

```rust
use systemprompt_database::DbPool;
use systemprompt_logging::{LoggingRepository, LogFilter, LogLevel};

async fn recent_errors(pool: &DbPool) -> anyhow::Result<()> {
    let repo = LoggingRepository::new(pool.clone());
    let filter = LogFilter::default().with_level(LogLevel::Error).with_limit(20);
    let entries = repo.list_logs_paginated(&filter).await?;
    for entry in entries {
        println!("{}: {}", entry.level, entry.message);
    }
    Ok(())
}
```

## Public API

### Initialization
- `init_logging(db_pool)` - Initialize with database persistence
- `init_console_logging()` - Initialize console-only logging

### Core Types
- `LogEntry` - Log entry with metadata and context IDs
- `LogLevel` - Enum: ERROR, WARN, INFO, DEBUG, TRACE
- `LogFilter` - Pagination and filtering for queries
- `LoggingError` - Error type for logging operations
- `DatabaseLayer` - Tracing layer for async database persistence

### Repositories
- `LoggingRepository` - CRUD operations for logs
- `AnalyticsRepository` - Analytics event tracking
- `AnalyticsEvent` - Structured analytics event

### Services
- `DatabaseLogService` - Implements `LogService` trait from systemprompt-traits
- `LoggingMaintenanceService` - Log cleanup and maintenance
- `CliService` - Rich CLI output facade
- `FilterSystemFields` - Console field filter

### Spans
- `RequestSpan` - For user-initiated operations
- `SystemSpan` - For internal/background operations
- `RequestSpanBuilder` - Fluent builder for request spans

### Retention
- `RetentionConfig` - Per-level retention configuration
- `RetentionPolicy` - Individual retention policy
- `RetentionScheduler` - Cron-based cleanup scheduler

### Trace Services
- `TraceQueryService` - Generic trace querying
- `AiTraceService` - AI/MCP operation tracing
- `TraceEvent`, `TaskInfo`, `ExecutionStep`, `AiRequestInfo`
- `McpToolExecution`, `ConversationMessage`, `ToolLogEntry`

### Output Control
- `is_startup_mode()`, `set_startup_mode()` - Startup mode control
- `publish_log()`, `set_log_publisher()` - Log event publishing

## Feature Flags

| Feature | Dependencies enabled |
|---------|---------------------|
| `cli` | `colored`, `console`, `dialoguer`, `indicatif` |

## Dependencies

### Internal
- `systemprompt-database` - Database pool management
- `systemprompt-traits` - Shared trait definitions (LogService)
- `systemprompt-identifiers` - Typed identifiers (UserId, SessionId, etc.)

### External
- `tracing`, `tracing-subscriber` - Structured logging framework
- `tokio` - Async runtime
- `sqlx` - Type-safe SQL
- `chrono` - Timestamp handling
- `tokio-cron-scheduler` - Retention job scheduling
- `colored`, `console`, `indicatif`, `dialoguer` - CLI utilities

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-logging)** · **[docs.rs](https://docs.rs/systemprompt-logging)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Infra layer · Own how your organization uses AI.</sub>

</div>
