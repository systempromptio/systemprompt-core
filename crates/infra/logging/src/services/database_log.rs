use async_trait::async_trait;
use systemprompt_core_database::DbPool;
use systemprompt_traits::LogService;

use crate::models::{LogEntry, LogFilter, LoggingError};
use crate::repository::LoggingRepository;

#[derive(Clone, Debug)]
pub struct DatabaseLogService {
    repository: LoggingRepository,
}

impl DatabaseLogService {
    #[must_use]
    pub const fn new(db_pool: DbPool) -> Self {
        Self {
            repository: LoggingRepository::new(db_pool)
                .with_terminal(false)
                .with_database(true),
        }
    }

    #[must_use]
    pub const fn from_repository(repository: LoggingRepository) -> Self {
        Self { repository }
    }

    #[must_use]
    pub const fn repository(&self) -> &LoggingRepository {
        &self.repository
    }
}

#[async_trait]
impl LogService for DatabaseLogService {
    type Entry = LogEntry;
    type Filter = LogFilter;
    type Error = LoggingError;

    async fn log(&self, entry: Self::Entry) -> Result<(), Self::Error> {
        self.repository.log(entry).await
    }

    async fn query(&self, filter: &Self::Filter) -> Result<(Vec<Self::Entry>, i64), Self::Error> {
        self.repository.get_logs_paginated(filter).await
    }

    async fn get_recent(&self, limit: i64) -> Result<Vec<Self::Entry>, Self::Error> {
        self.repository.get_recent_logs(limit).await
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Self::Entry>, Self::Error> {
        let log_id = systemprompt_identifiers::LogId::new(id);
        self.repository.get_by_id(&log_id).await
    }

    async fn delete(&self, id: &str) -> Result<bool, Self::Error> {
        let log_id = systemprompt_identifiers::LogId::new(id);
        self.repository.delete_log_entry(&log_id).await
    }
}
