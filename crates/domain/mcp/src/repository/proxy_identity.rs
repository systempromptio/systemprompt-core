//! Persistence for the proxy-side MCP session identity.
//!
//! [`McpProxyIdentityRepository`] stores the identity established on an
//! authenticated `initialize` call, keyed by `mcp-session-id`, so a
//! session-only follow-up resolves the same identity on every replica. It is
//! the trust anchor for session-based MCP auth: lookups read the write pool
//! because a replica-lag miss would downgrade a verified caller to anonymous.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_models::auth::{Permission, UserType};

#[derive(Debug, Clone)]
pub struct ProxyIdentityRow {
    pub user_id: UserId,
    pub user_type: UserType,
    pub permissions: Vec<Permission>,
    pub auth_token: JwtToken,
}

#[derive(Debug, Clone)]
pub struct McpProxyIdentityRepository {
    write_pool: Arc<PgPool>,
}

impl McpProxyIdentityRepository {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| McpDomainError::Internal(format!("Database must be PostgreSQL: {e}")))?;
        Ok(Self { write_pool })
    }

    pub async fn upsert(
        &self,
        session_id: &SessionId,
        identity: &ProxyIdentityRow,
    ) -> McpDomainResult<()> {
        let permissions = serde_json::to_value(&identity.permissions)?;
        sqlx::query!(
            r#"
            INSERT INTO mcp_proxy_identities
                (session_id, user_id, user_type, permissions, auth_token)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (session_id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                user_type = EXCLUDED.user_type,
                permissions = EXCLUDED.permissions,
                auth_token = EXCLUDED.auth_token,
                expires_at = NOW() + INTERVAL '24 hours'
            "#,
            session_id.as_str(),
            identity.user_id.as_str(),
            identity.user_type.as_str(),
            permissions,
            identity.auth_token.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn find(&self, session_id: &SessionId) -> McpDomainResult<Option<ProxyIdentityRow>> {
        let row = sqlx::query!(
            r#"
            SELECT
                user_id as "user_id!: UserId",
                user_type,
                permissions,
                auth_token
            FROM mcp_proxy_identities
            WHERE session_id = $1
              AND expires_at > NOW()
            "#,
            session_id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?;

        row.map(|r| {
            let user_type = UserType::from_str(&r.user_type)
                .map_err(|e| McpDomainError::Validation(e.to_string()))?;
            let permissions: Vec<Permission> = serde_json::from_value(r.permissions)?;
            Ok(ProxyIdentityRow {
                user_id: r.user_id,
                user_type,
                permissions,
                auth_token: JwtToken::new(r.auth_token),
            })
        })
        .transpose()
    }

    pub async fn delete(&self, session_id: &SessionId) -> McpDomainResult<()> {
        sqlx::query!(
            r#"DELETE FROM mcp_proxy_identities WHERE session_id = $1"#,
            session_id.as_str()
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn cleanup_expired(&self) -> McpDomainResult<u64> {
        let result = sqlx::query!(r#"DELETE FROM mcp_proxy_identities WHERE expires_at <= NOW()"#)
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }
}
