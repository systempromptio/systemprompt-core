# systemprompt-logging

[![Crates.io](https://img.shields.io/crates/v/systemprompt-logging.svg?style=flat-square)](https://crates.io/crates/systemprompt-logging)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-logging?style=flat-square)](https://docs.rs/systemprompt-logging)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Provides structured tracing, PostgreSQL log persistence and typed audit queries. Record integrity depends on database permissions and retention controls.

**Layer**: Infra. Infrastructure primitives consumed by the domain and application crates. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## What it does

A `tracing` layer batches structured events into PostgreSQL asynchronously, propagating context (`user_id`, `session_id`, `task_id`, `trace_id`) onto every row so the audit trail reconstructs who did what. A cron scheduler applies per-level retention. On top of the stored trail sits a typed query surface over traces, AI requests, and MCP tool executions. The `cli` feature adds console formatting for the CLI.

## Modules

| Module | Purpose |
|--------|---------|
| `layer` | `DatabaseLayer` (async batch persistence) and `ProxyDatabaseLayer` (late-bound sink swap-in), plus field/span visitors. |
| `models` | `LogEntry`, `LogLevel`, `LogFilter`, `LogActor`, the `LoggingError` type, and the `LogRow` SQLx mapping. |
| `repository` | `LoggingRepository` (log CRUD) and `AnalyticsRepository` / `AnalyticsEvent`. |
| `services` | `DatabaseLogService`, `LoggingMaintenanceService`, retention scheduling, request/system spans, startup-mode and log-publisher control, and the `cli/` display helpers (feature-gated). |
| `trace` | `TraceQueryService` and `AiTraceService` with the typed rows for traces, AI requests, and MCP tool executions. |
| `attribution` | Installs the process-wide system-actor attribution used to stamp platform-originated log rows (`install_log_attribution`, `platform_attribution`). |
| `sanitize` | Internal helpers that scrub sensitive fields before rows are persisted. |
| `extension` | `LoggingExtension` registers the `logs` and `analytics_events` schema through the workspace extension framework. |

The `cli` submodule under `services/` provides `banners`, `display`, `macros`, `service` (`CliService`), `startup`, `table`, `theme`, and `types`. Schema DDL lives in `schema/` (`log.sql`, `analytics.sql`, and the `migrations/` directory).

## Usage

```toml
[dependencies]
systemprompt-logging = "0.49"
```

```rust
use systemprompt_database::DbPool;
use systemprompt_logging::{LoggingRepository, LogFilter, LogLevel};

use systemprompt_logging::LoggingError;

async fn recent_errors(pool: &DbPool) -> Result<(), LoggingError> {
    let repo = LoggingRepository::new(pool.clone());
    let filter = LogFilter::default().with_level(LogLevel::Error).with_limit(20);
    let entries = repo.list_logs_paginated(&filter).await?;
    for entry in entries {
        tracing::info!(level = %entry.level, message = %entry.message, "log");
    }
    Ok(())
}
```

## Public API

### Initialization
- `init_logging(db_pool)` - Initialize with database persistence
- `init_console_logging()` - Initialize console-only logging
- `init_console_logging_with_level(level)` - Initialize console logging at a specific level
- `LoggingExtension` - Inventory-registered schema/extension entry point

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

### Attribution
- `install_log_attribution()`, `platform_attribution()` - Process-wide system-actor stamping for platform-originated rows

## Feature Flags

| Feature | Dependencies enabled |
|---------|---------------------|
| `cli` | `console`, `indicatif` |

## Dependencies

### Internal
- `systemprompt-database` - Database pool management
- `systemprompt-extension` - Extension framework (schema registration)
- `systemprompt-identifiers` - Typed identifiers (UserId, SessionId, etc.)
- `systemprompt-models` - Shared model types
- `systemprompt-traits` - Shared trait definitions (`LogService`)

### External
- `tracing`, `tracing-subscriber` - Structured logging framework
- `tokio` - Async runtime
- `sqlx` - Type-safe SQL
- `serde`, `serde_json`, `serde_yaml` - Serialisation
- `chrono` - Timestamp handling
- `uuid` - Log entry identifiers
- `tokio-cron-scheduler` - Retention job scheduling
- `async-trait`, `thiserror`, `inventory` - Trait, error, and registration utilities
- `console`, `indicatif` - CLI utilities (`cli` feature)

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
