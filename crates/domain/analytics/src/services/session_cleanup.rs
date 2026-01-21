use anyhow::Result;
use systemprompt_database::DbPool;

use crate::repository::SessionRepository;

#[derive(Clone, Debug)]
pub struct SessionCleanupService {
    session_repo: SessionRepository,
}

impl SessionCleanupService {
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            session_repo: SessionRepository::new(db_pool),
        }
    }

    pub async fn cleanup_inactive_sessions(&self, inactive_hours: i32) -> Result<u64> {
        self.session_repo.cleanup_inactive(inactive_hours).await
    }
}
