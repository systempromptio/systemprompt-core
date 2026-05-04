//! Periodic cleanup queries for orphaned rows and expired tokens.

use anyhow::Result;
use sqlx::PgPool;

/// Repository owning destructive housekeeping queries (`DELETE` over expired
/// or orphaned rows). All methods take the write pool.
#[derive(Debug)]
pub struct CleanupRepository {
    write_pool: PgPool,
}

impl CleanupRepository {
    /// Construct from a write pool.
    pub const fn new(pool: PgPool) -> Self {
        Self { write_pool: pool }
    }

    /// Construct from an explicit write pool. Equivalent to [`Self::new`];
    /// kept for call-site clarity at write/read split boundaries.
    pub const fn new_with_write_pool(write_pool: PgPool) -> Self {
        Self { write_pool }
    }

    /// Delete log rows whose `user_id` no longer references a live user.
    pub async fn delete_orphaned_logs(&self) -> Result<u64> {
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

    /// Delete MCP tool execution rows whose `context_id` no longer
    /// references a live user context.
    pub async fn delete_orphaned_mcp_executions(&self) -> Result<u64> {
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

    /// Delete log rows older than `days`.
    pub async fn delete_old_logs(&self, days: i32) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        let result = sqlx::query!("DELETE FROM logs WHERE timestamp < $1", cutoff)
            .execute(&self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete OAuth refresh tokens whose `expires_at` is in the past.
    pub async fn delete_expired_oauth_tokens(&self) -> Result<u64> {
        let result = sqlx::query!("DELETE FROM oauth_refresh_tokens WHERE expires_at < NOW()")
            .execute(&self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete OAuth authorization codes that are expired or already redeemed.
    pub async fn delete_expired_oauth_codes(&self) -> Result<u64> {
        let result = sqlx::query!(
            "DELETE FROM oauth_auth_codes WHERE expires_at < NOW() OR used_at IS NOT NULL"
        )
        .execute(&self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }
}
