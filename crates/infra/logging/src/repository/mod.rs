#![allow(clippy::print_stdout)]

use chrono::{DateTime, Utc};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::LogId;

use crate::models::{LogEntry, LogFilter, LogLevel, LoggingError};

pub mod analytics;
mod operations;

pub use analytics::{AnalyticsEvent, AnalyticsRepository};

#[derive(Clone, Debug)]
pub struct LoggingRepository {
    db_pool: DbPool,
    terminal_output: bool,
    db_output: bool,
}

impl LoggingRepository {
    #[must_use]
    pub const fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            terminal_output: true,
            db_output: false,
        }
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
            println!("{entry}");
        }

        if self.db_output {
            operations::create_log(&self.db_pool, &entry).await?;
        }

        Ok(())
    }

    pub async fn log_message(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
    ) -> Result<(), LoggingError> {
        let entry = LogEntry::new(level, module, message);
        self.log(entry).await
    }

    pub async fn log_message_with_metadata(
        &self,
        level: LogLevel,
        module: &str,
        message: &str,
        metadata: serde_json::Value,
    ) -> Result<(), LoggingError> {
        let entry = LogEntry::new(level, module, message).with_metadata(metadata);
        self.log(entry).await
    }

    pub async fn error(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.log_message(LogLevel::Error, module, message).await
    }

    pub async fn warn(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.log_message(LogLevel::Warn, module, message).await
    }

    pub async fn info(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.log_message(LogLevel::Info, module, message).await
    }

    pub async fn debug(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.log_message(LogLevel::Debug, module, message).await
    }

    pub async fn trace(&self, module: &str, message: &str) -> Result<(), LoggingError> {
        self.log_message(LogLevel::Trace, module, message).await
    }

    pub async fn get_recent_logs(&self, limit: i64) -> Result<Vec<LogEntry>, LoggingError> {
        operations::list_logs(&self.db_pool, limit).await
    }

    pub async fn get_logs_by_module_patterns(
        &self,
        patterns: &[String],
        limit: i64,
    ) -> Result<Vec<LogEntry>, LoggingError> {
        operations::list_logs_by_module_patterns(&self.db_pool, patterns, limit).await
    }

    pub async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> Result<u64, LoggingError> {
        operations::cleanup_logs_before(&self.db_pool, older_than).await
    }

    pub async fn clear_all_logs(&self) -> Result<u64, LoggingError> {
        operations::clear_all_logs(&self.db_pool).await
    }

    pub async fn get_logs_paginated(
        &self,
        filter: &LogFilter,
    ) -> Result<(Vec<LogEntry>, i64), LoggingError> {
        operations::list_logs_paginated(&self.db_pool, filter).await
    }

    pub async fn get_by_id(&self, id: &LogId) -> Result<Option<LogEntry>, LoggingError> {
        operations::get_log(&self.db_pool, id).await
    }

    pub async fn update_log_entry(
        &self,
        id: &LogId,
        entry: &LogEntry,
    ) -> Result<bool, LoggingError> {
        operations::update_log(&self.db_pool, id, entry).await
    }

    pub async fn delete_log_entry(&self, id: &LogId) -> Result<bool, LoggingError> {
        operations::delete_log(&self.db_pool, id).await
    }

    pub async fn delete_log_entries(&self, ids: &[LogId]) -> Result<u64, LoggingError> {
        operations::delete_logs_multiple(&self.db_pool, ids).await
    }
}
