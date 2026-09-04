//! Bulk user-record mutations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::models::UserStatus;
use crate::repository::UserRepository;

impl UserRepository {
    pub async fn bulk_update_status(&self, user_ids: &[UserId], new_status: &str) -> Result<u64> {
        let ids: Vec<String> = user_ids.iter().map(ToString::to_string).collect();
        let result = sqlx::query!(
            r#"
            UPDATE users
            SET status = $1, updated_at = NOW()
            WHERE id = ANY($2)
            "#,
            new_status,
            &ids[..]
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn bulk_delete(&self, user_ids: &[UserId]) -> Result<u64> {
        let deleted_status = UserStatus::Deleted.as_str();
        let ids: Vec<String> = user_ids.iter().map(ToString::to_string).collect();
        let result = sqlx::query!(
            r#"
            UPDATE users
            SET status = $1, updated_at = NOW()
            WHERE id = ANY($2)
            "#,
            deleted_status,
            &ids[..]
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
