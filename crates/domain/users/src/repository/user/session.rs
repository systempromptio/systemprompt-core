use chrono::Utc;
use systemprompt_identifiers::{SessionId, UserId};

use crate::error::Result;
use crate::models::{UserSession, UserSessionRow};
use crate::repository::{UserRepository, MAX_PAGE_SIZE};

impl UserRepository {
    pub async fn list_sessions(&self, user_id: &UserId) -> Result<Vec<UserSession>> {
        let rows = sqlx::query_as!(
            UserSessionRow,
            r#"
            SELECT session_id, user_id, ip_address, user_agent, device_type,
                   started_at, last_activity_at, ended_at
            FROM user_sessions
            WHERE user_id = $1
            ORDER BY last_activity_at DESC
            "#,
            user_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(UserSession::from).collect())
    }

    pub async fn list_active_sessions(&self, user_id: &UserId) -> Result<Vec<UserSession>> {
        let rows = sqlx::query_as!(
            UserSessionRow,
            r#"
            SELECT session_id, user_id, ip_address, user_agent, device_type,
                   started_at, last_activity_at, ended_at
            FROM user_sessions
            WHERE user_id = $1 AND ended_at IS NULL
            ORDER BY last_activity_at DESC
            "#,
            user_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(UserSession::from).collect())
    }

    pub async fn list_recent_sessions(
        &self,
        user_id: &UserId,
        limit: i64,
    ) -> Result<Vec<UserSession>> {
        let safe_limit = limit.min(MAX_PAGE_SIZE);
        let rows = sqlx::query_as!(
            UserSessionRow,
            r#"
            SELECT session_id, user_id, ip_address, user_agent, device_type,
                   started_at, last_activity_at, ended_at
            FROM user_sessions
            WHERE user_id = $1
            ORDER BY last_activity_at DESC
            LIMIT $2
            "#,
            user_id.as_str(),
            safe_limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(UserSession::from).collect())
    }

    pub async fn end_session(&self, session_id: &SessionId) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE user_sessions
            SET ended_at = $1
            WHERE session_id = $2 AND ended_at IS NULL
            "#,
            Utc::now(),
            session_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn end_all_sessions(&self, user_id: &UserId) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            UPDATE user_sessions
            SET ended_at = $1
            WHERE user_id = $2 AND ended_at IS NULL
            "#,
            Utc::now(),
            user_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
