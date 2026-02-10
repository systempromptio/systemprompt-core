use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::repository::UserRepository;

#[derive(Debug, Clone, Copy)]
pub struct MergeResult {
    pub sessions_transferred: u64,
    pub tasks_transferred: u64,
}

impl UserRepository {
    pub async fn merge_users(&self, source_id: &UserId, target_id: &UserId) -> Result<MergeResult> {
        let sessions_result = sqlx::query!(
            r#"
            UPDATE user_sessions
            SET user_id = $1
            WHERE user_id = $2
            "#,
            target_id.as_str(),
            source_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        let tasks_result = sqlx::query!(
            r#"
            UPDATE agent_tasks
            SET user_id = $1
            WHERE user_id = $2
            "#,
            target_id.as_str(),
            source_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        sqlx::query!(r#"DELETE FROM users WHERE id = $1"#, source_id.as_str())
            .execute(&*self.write_pool)
            .await?;

        Ok(MergeResult {
            sessions_transferred: sessions_result.rows_affected(),
            tasks_transferred: tasks_result.rows_affected(),
        })
    }
}
