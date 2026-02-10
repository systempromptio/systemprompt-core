use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct McpSessionRecord {
    pub session_id: String,
    pub user_id: Option<String>,
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
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db
            .pool_arc()
            .map_err(|e| anyhow::anyhow!("Database must be PostgreSQL: {e}"))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| anyhow::anyhow!("Database must be PostgreSQL: {e}"))?;
        Ok(Self { pool, write_pool })
    }

    pub async fn create(
        &self,
        session_id: &str,
        user_id: Option<&str>,
        mcp_server_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO mcp_sessions (session_id, user_id, mcp_server_id, status)
            VALUES ($1, $2, $3, 'active')
            ON CONFLICT (session_id) DO NOTHING
            "#,
            session_id,
            user_id,
            mcp_server_id,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn exists(&self, session_id: &str) -> Result<bool> {
        let result = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM mcp_sessions WHERE session_id = $1) as "exists!""#,
            session_id
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(result)
    }

    pub async fn find_active(&self, session_id: &str) -> Result<Option<McpSessionRecord>> {
        let row = sqlx::query!(
            r#"
            SELECT
                session_id as "session_id!",
                user_id,
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
            session_id
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

    pub async fn update_last_event_id(&self, session_id: &str, last_event_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET last_event_id = $2, last_activity_at = NOW()
            WHERE session_id = $1
            "#,
            session_id,
            last_event_id,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn update_activity(&self, session_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET last_activity_at = NOW()
            WHERE session_id = $1
            "#,
            session_id,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn close(&self, session_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE mcp_sessions
            SET status = 'closed', last_activity_at = NOW()
            WHERE session_id = $1
            "#,
            session_id,
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
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
