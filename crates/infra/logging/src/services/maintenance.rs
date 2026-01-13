use chrono::{DateTime, Utc};
use systemprompt_core_database::DbPool;

use crate::models::{LogEntry, LoggingError};
use crate::repository::LoggingRepository;

#[derive(Clone, Debug)]
pub struct LoggingMaintenanceService {
    repo: LoggingRepository,
}

impl LoggingMaintenanceService {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            repo: LoggingRepository::new(db_pool),
        }
    }

    pub async fn get_recent_logs(&self, limit: i64) -> Result<Vec<LogEntry>, LoggingError> {
        self.repo.get_recent_logs(limit).await
    }

    pub async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> Result<u64, LoggingError> {
        self.repo.cleanup_old_logs(older_than).await
    }

    pub async fn count_logs_before(&self, cutoff: DateTime<Utc>) -> Result<u64, LoggingError> {
        self.repo.count_logs_before(cutoff).await
    }

    pub async fn clear_all_logs(&self) -> Result<u64, LoggingError> {
        self.repo.clear_all_logs().await
    }

    pub const fn vacuum() {}
}
