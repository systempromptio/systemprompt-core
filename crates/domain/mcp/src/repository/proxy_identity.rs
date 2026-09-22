//! Persistence for the proxy-side MCP session identity.
//!
//! [`McpProxyIdentityRepository`] stores the identity established on an
//! authenticated `initialize` call, keyed by `mcp-session-id`, so a
//! session-only follow-up resolves the same identity on every replica. It is
//! the trust anchor for session-based MCP auth: lookups read the write pool
//! because a replica-lag miss would downgrade a verified caller to anonymous.
//!
//! The row carries the caller's bearer JWT because the proxy replays it to the
//! upstream MCP server, so it cannot be hashed. It is sealed instead
//! ([`systemprompt_security::at_rest`], ChaCha20-Poly1305 under
//! `encryption_master_key`), and a row that does not open is dropped rather
//! than trusted: a 24-hour identity cache is cheap to rebuild on the next
//! `initialize`, and a token we cannot authenticate is not one to forward.
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
    pub roles: Vec<String>,
    pub auth_token: JwtToken,
}

#[derive(Debug, Clone)]
pub struct McpProxyIdentityRepository {
    pub(super) write_pool: Arc<PgPool>,
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
        let roles = serde_json::to_value(&identity.roles)?;
        let auth_token = systemprompt_security::at_rest::seal(identity.auth_token.as_str())
            .map_err(|e| McpDomainError::Internal(format!("Sealing proxy identity token: {e}")))?;
        sqlx::query!(
            r#"
            INSERT INTO mcp_proxy_identities
                (session_id, user_id, user_type, permissions, roles, auth_token)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (session_id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                user_type = EXCLUDED.user_type,
                permissions = EXCLUDED.permissions,
                roles = EXCLUDED.roles,
                auth_token = EXCLUDED.auth_token,
                expires_at = NOW() + INTERVAL '24 hours'
            "#,
            session_id.as_str(),
            identity.user_id.as_str(),
            identity.user_type.as_str(),
            permissions,
            roles,
            auth_token.as_str(),
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
                roles,
                auth_token
            FROM mcp_proxy_identities
            WHERE session_id = $1
              AND expires_at > NOW()
            "#,
            session_id.as_str()
        )
        .fetch_optional(&*self.write_pool)
        .await?;

        let Some(r) = row else {
            return Ok(None);
        };
        let Ok(auth_token) = systemprompt_security::at_rest::open(&r.auth_token) else {
            tracing::warn!(
                session = %session_id,
                "Proxy identity token did not open; dropping the row so the next initialize \
                 re-establishes the identity"
            );
            self.delete(session_id).await?;
            return Ok(None);
        };
        let user_type = UserType::from_str(&r.user_type)
            .map_err(|e| McpDomainError::Validation(e.to_string()))?;
        let permissions: Vec<Permission> = serde_json::from_value(r.permissions)?;
        let roles: Vec<String> = serde_json::from_value(r.roles)?;
        Ok(Some(ProxyIdentityRow {
            user_id: r.user_id,
            user_type,
            permissions,
            roles,
            auth_token: JwtToken::new(auth_token),
        }))
    }

    // Why: the backend opens the `mcp_sessions` row before it knows who is
    // calling; the proxy is the first party that knows both the server it
    // routed to and the verified user, so it fills the two attribution columns
    // the console groups by. `COALESCE` keeps whatever the server stamped.
    pub async fn attribute_session(
        &self,
        session_id: &SessionId,
        server_name: &str,
        user_id: &UserId,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"UPDATE mcp_sessions
               SET mcp_server_id = COALESCE(mcp_server_id, $2),
                   user_id = COALESCE(user_id, $3)
               WHERE session_id = $1"#,
            session_id.as_str(),
            server_name,
            user_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
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
        sqlx::query!("DELETE FROM mcp_external_sessions WHERE expires_at <= NOW()")
            .execute(&*self.write_pool)
            .await?;
        let result = sqlx::query!(r#"DELETE FROM mcp_proxy_identities WHERE expires_at <= NOW()"#)
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }
}
