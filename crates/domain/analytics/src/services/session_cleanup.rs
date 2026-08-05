//! Inactive-session cleanup service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;

use crate::repository::SessionRepository;

#[derive(Clone, Debug)]
pub struct SessionCleanupService {
    session_repo: SessionRepository,
}

impl SessionCleanupService {
    pub const fn new(session_repo: SessionRepository) -> Self {
        Self { session_repo }
    }

    pub async fn cleanup_inactive_sessions(&self, inactive_hours: i32) -> Result<u64> {
        self.session_repo.cleanup_inactive(inactive_hours).await
    }
}
