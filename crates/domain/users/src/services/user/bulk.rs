//! Counts, statistics and filtered bulk operations over user accounts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::UserId;

use super::UserService;
use crate::error::Result;
use crate::models::{User, UserCountBreakdown, UserStats, UserStatus};

impl UserService {
    pub async fn count_with_breakdown(&self) -> Result<UserCountBreakdown> {
        let total = self.repository.count().await?;
        let by_status_vec = self.repository.count_by_status().await?;
        let by_role_vec = self.repository.count_by_role().await?;

        let by_status: HashMap<String, i64> = by_status_vec.into_iter().collect();
        let by_role: HashMap<String, i64> = by_role_vec.into_iter().collect();

        Ok(UserCountBreakdown {
            total,
            by_status,
            by_role,
        })
    }

    pub async fn get_stats(&self) -> Result<UserStats> {
        self.repository.get_stats().await
    }

    pub async fn list_by_filter(
        &self,
        status: Option<UserStatus>,
        role: Option<&str>,
        older_than_days: Option<i64>,
        limit: i64,
    ) -> Result<Vec<User>> {
        self.repository
            .list_by_filter(status, role, older_than_days, limit)
            .await
    }

    pub async fn bulk_update_status(
        &self,
        user_ids: &[UserId],
        new_status: UserStatus,
    ) -> Result<u64> {
        self.repository
            .bulk_update_status(user_ids, new_status)
            .await
    }

    pub async fn bulk_delete(&self, user_ids: &[UserId]) -> Result<u64> {
        self.repository.bulk_delete(user_ids).await
    }
}
