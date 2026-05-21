//! Periodic cleanup queries for orphaned rows and expired tokens.

use sqlx::PgPool;

use crate::error::DatabaseResult;

#[derive(Debug)]
pub struct CleanupRepository {
    write_pool: PgPool,
}

impl CleanupRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { write_pool: pool }
    }

    pub const fn new_with_write_pool(write_pool: PgPool) -> Self {
        Self { write_pool }
    }

    pub async fn delete_orphaned_logs(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM logs
            WHERE user_id IS NOT NULL
              AND user_id NOT IN (SELECT id FROM users)
            "#
        )
        .execute(&self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_orphaned_mcp_executions(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM mcp_tool_executions
            WHERE context_id IS NOT NULL
              AND context_id NOT IN (SELECT context_id FROM user_contexts)
            "#
        )
        .execute(&self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_old_logs(&self, days: i32) -> DatabaseResult<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        let result = sqlx::query!("DELETE FROM logs WHERE timestamp < $1", cutoff)
            .execute(&self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn count_old_logs(&self, days: i32) -> DatabaseResult<i64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM logs WHERE timestamp < $1"#,
            cutoff
        )
        .fetch_one(&self.write_pool)
        .await?;
        Ok(count)
    }

    pub async fn delete_expired_oauth_tokens(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_refresh_tokens WHERE expires_at < NOW()")
            .execute(&self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_oauth_codes(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM oauth_auth_codes WHERE expires_at < NOW() OR used_at IS NOT NULL"
        )
        .execute(&self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_oauth_state_bindings(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_state_bindings WHERE expires_at < NOW()")
            .execute(&self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_oauth_jti_revocations(&self) -> DatabaseResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_jti_revocations WHERE exp < NOW()")
            .execute(&self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }
}
