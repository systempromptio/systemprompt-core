//! Log persistence repository.
//!
//! [`LoggingRepository`] writes entries to the configured sinks (terminal
//! and/or database) and serves paginated reads, lookups, and age-based cleanup;
//! [`AnalyticsRepository`] records analytics events. Read and write pools are
//! held separately so reads never contend with the write path.

use std::io::Write;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::LogId;

use crate::models::{LogEntry, LogFilter, LoggingError};

pub mod analytics;
mod operations;

pub use analytics::{AnalyticsEvent, AnalyticsRepository};

#[derive(Clone, Debug)]
pub struct LoggingRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    terminal_output: bool,
    db_output: bool,
}

impl LoggingRepository {
    pub fn new(db: &DbPool) -> Result<Self, LoggingError> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self {
            pool,
            write_pool,
            terminal_output: true,
            db_output: false,
        })
    }

    #[must_use]
    pub const fn with_terminal(mut self, enabled: bool) -> Self {
        self.terminal_output = enabled;
        self
    }

    #[must_use]
    pub const fn with_database(mut self, enabled: bool) -> Self {
        self.db_output = enabled;
        self
    }

    pub async fn log(&self, entry: LogEntry) -> Result<(), LoggingError> {
        entry.validate()?;

        if self.terminal_output {
            let mut stdout = std::io::stdout();
            writeln!(stdout, "{entry}").ok();
        }

        if self.db_output {
            operations::create_log(&self.write_pool, &entry).await?;
        }

        Ok(())
    }

    pub async fn get_recent_logs(&self, limit: i64) -> Result<Vec<LogEntry>, LoggingError> {
        operations::list_logs(&self.pool, limit).await
    }

    pub async fn get_logs_by_module_patterns(
        &self,
        patterns: &[String],
        limit: i64,
    ) -> Result<Vec<LogEntry>, LoggingError> {
        operations::list_logs_by_module_patterns(&self.pool, patterns, limit).await
    }

    pub async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> Result<u64, LoggingError> {
        operations::cleanup_logs_before(&self.write_pool, older_than).await
    }

    pub async fn count_logs_before(&self, cutoff: DateTime<Utc>) -> Result<u64, LoggingError> {
        operations::count_logs_before(&self.pool, cutoff).await
    }

    pub async fn clear_all_logs(&self) -> Result<u64, LoggingError> {
        operations::clear_all_logs(&self.write_pool).await
    }

    pub async fn get_logs_paginated(
        &self,
        filter: &LogFilter,
    ) -> Result<(Vec<LogEntry>, i64), LoggingError> {
        operations::list_logs_paginated(&self.pool, filter).await
    }

    pub async fn get_by_id(&self, id: &LogId) -> Result<Option<LogEntry>, LoggingError> {
        operations::get_log(&self.pool, id).await
    }

    pub async fn update_log_entry(
        &self,
        id: &LogId,
        entry: &LogEntry,
    ) -> Result<bool, LoggingError> {
        operations::update_log(&self.write_pool, id, entry).await
    }

    pub async fn delete_log_entry(&self, id: &LogId) -> Result<bool, LoggingError> {
        operations::delete_log(&self.write_pool, id).await
    }

    pub async fn delete_log_entries(&self, ids: &[LogId]) -> Result<u64, LoggingError> {
        operations::delete_logs_multiple(&self.write_pool, ids).await
    }
}
