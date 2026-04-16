<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://systemprompt.io/documentation">Documentation</a> • <a href="https://github.com/systempromptio/systemprompt-core">Core</a> • <a href="https://github.com/systempromptio/systemprompt-template">Template</a></p>
</div>

---


# systemprompt-logging

Core logging module for systemprompt.io OS.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-logging.svg)](https://crates.io/crates/systemprompt-logging)
[![Documentation](https://docs.rs/systemprompt-logging/badge.svg)](https://docs.rs/systemprompt-logging)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

## Overview

**Part of the Infra layer in the systemprompt.io architecture.**
**Capabilities** · [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)

This crate provides a dual-layer logging architecture combining console output with PostgreSQL persistence. It includes async batch processing, automatic context propagation, retention policies, and rich CLI output utilities.

**Key Features:**
- Dual-layer logging (console + database)
- Async batch processing with configurable flush intervals
- Automatic context propagation (user_id, session_id, task_id, trace_id)
- Cron-based log retention with per-level policies
- AI/MCP operation tracing and querying
- Rich CLI output with themes and progress indicators

## Structure

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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-logging = "0.0.1"
```

## Usage

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

## License

Business Source License 1.1 - See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE) for details.
