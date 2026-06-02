//! Persistence for MCP transport sessions.
//!
//! Defines [`McpSessionRepository`] and the [`McpSessionRecord`] row model over
//! the `mcp_sessions` table, tracking session lifecycle (active → expired →
//! closed), activity timestamps, and last-event-id for SSE resumption.

use crate::error::McpDomainResult;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};

#[derive(Debug, Clone)]
pub struct McpSessionRecord {
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub mcp_server_id: Option<String>,
    pub last_event_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct McpSessionRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl McpSessionRepository {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let pool = db.pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        let write_pool = db.write_pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        Ok(Self { pool, write_pool })
    }

    pub async fn create(
        &self,
        session_id: &SessionId,
        user_id: Option<&UserId>,
        mcp_server_id: Option<&str>,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_sessions (session_id, user_id, mcp_server_id, status)
            VALUES ($1, $2, $3, 'active')
            ON CONFLICT (session_id) DO NOTHING
            "#,
            session_id.as_str(),
            user_id.map(UserId::as_str),
            mcp_server_id,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn exists(&self, session_id: &SessionId) -> McpDomainResult<bool> {
        let result = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_sessions WHERE session_id = $1) as "exists!""#,
            session_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(result)
    }

    pub async fn find_active(
        &self,
        session_id: &SessionId,
    ) -> McpDomainResult<Option<McpSessionRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT
                session_id as "session_id!: SessionId",
                user_id as "user_id: UserId",
                mcp_server_id,
                last_event_id,
                status as "status!",
                created_at as "created_at!",
                last_activity_at as "last_activity_at!",
                expires_at as "expires_at!"
            FROM mcp_sessions
            WHERE session_id = $1
              AND status = 'active'
              AND expires_at > NOW()
            "#,
            session_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| McpSessionRecord {
            session_id: r.session_id,
            user_id: r.user_id,
            mcp_server_id: r.mcp_server_id,
            last_event_id: r.last_event_id,
            status: r.status,
            created_at: r.created_at,
            last_activity_at: r.last_activity_at,
            expires_at: r.expires_at,
        }))
    }

    pub async fn update_last_event_id(
        &self,
        session_id: &SessionId,
        last_event_id: &str,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET last_event_id = $2, last_activity_at = NOW()
            WHERE session_id = $1
            "#,
            session_id.as_str(),
            last_event_id,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn update_activity(&self, session_id: &SessionId) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET last_activity_at = NOW(),
                expires_at = NOW() + INTERVAL '24 hours',
                status = 'active'
            WHERE session_id = $1
            "#,
            session_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn store_initialize_params(
        &self,
        session_id: &SessionId,
        params: &serde_json::Value,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_sessions (session_id, initialize_params, status)
            VALUES ($1, $2, 'active')
            ON CONFLICT (session_id)
            DO UPDATE SET initialize_params = EXCLUDED.initialize_params
            "#,
            session_id.as_str(),
            params,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn find_initialize_params(
        &self,
        session_id: &SessionId,
    ) -> McpDomainResult<Option<serde_json::Value>> {
        // A worker that ends (after any request, or a server restart) marks its
        // row 'closed' but leaves initialize_params intact; only a client DELETE
        // nulls them. Presence of non-null params — not status — is the
        // recoverable signal, so recovery must not filter on status = 'active'.
        let row = sqlx::query!(
            r#"
            SELECT initialize_params
            FROM mcp_sessions
            WHERE session_id = $1
              AND expires_at > NOW()
              AND initialize_params IS NOT NULL
            "#,
            session_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.and_then(|r| r.initialize_params))
    }

    pub async fn clear_initialize_params(&self, session_id: &SessionId) -> McpDomainResult<()> {
        sqlx::query!(
            r#"UPDATE mcp_sessions SET initialize_params = NULL WHERE session_id = $1"#,
            session_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn close(&self, session_id: &SessionId) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET status = 'closed', last_activity_at = NOW()
            WHERE session_id = $1
            "#,
            session_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn delete_stale(&self, retention_days: i32) -> McpDomainResult<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM mcp_sessions
            WHERE status IN ('expired', 'closed')
              AND last_activity_at < NOW() - make_interval(days => $1)
            "#,
            retention_days,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn cleanup_expired(&self) -> McpDomainResult<u64> {
        let result = sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET status = 'expired'
            WHERE status = 'active' AND expires_at < NOW()
            "#,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
