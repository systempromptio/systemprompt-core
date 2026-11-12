//! Proxy-side binding of MCP sessions to the server and principal that opened
//! them.
//!
//! [`McpProxySessionRepository`] records, on the `mcp_sessions` row, which
//! server a managed session was opened on and which authenticated principal
//! opened it, and answers whether a later request may use that session id.
//! The session id is only a correlation handle: identity always comes from the
//! request's own verified credential, never from the session. Reads go to the
//! write pool so a replica-lag miss cannot refuse a session just opened. The
//! same repository holds the external-server bindings
//! (`mcp_external_sessions`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{McpServerId, SessionId, UserId};

#[derive(Debug, Clone)]
pub struct McpProxySessionRepository {
    pub(super) write_pool: Arc<PgPool>,
}

impl McpProxySessionRepository {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }

    // Why: the backend opens the `mcp_sessions` row before it knows who is
    // calling, and may not have written it yet when the proxy sees the
    // `initialize` response; the proxy is the first party that knows both the
    // server it routed to and the verified principal. `COALESCE` keeps a
    // binding already recorded, so a later caller cannot rebind a session.
    pub async fn bind(
        &self,
        session_id: &SessionId,
        server: &McpServerId,
        principal: Option<&UserId>,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"INSERT INTO mcp_sessions (session_id, mcp_server_id, user_id, status)
               VALUES ($1, $2, $3, 'active')
               ON CONFLICT (session_id) DO UPDATE
               SET mcp_server_id = COALESCE(mcp_sessions.mcp_server_id, EXCLUDED.mcp_server_id),
                   user_id = COALESCE(mcp_sessions.user_id, EXCLUDED.user_id)"#,
            session_id.as_str(),
            server.as_str(),
            principal.map(UserId::as_str),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn is_bound_to(
        &self,
        session_id: &SessionId,
        server: &McpServerId,
        principal: Option<&UserId>,
    ) -> McpDomainResult<bool> {
        let bound = sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM mcp_sessions
                   WHERE session_id = $1
                     AND mcp_server_id = $2
                     AND user_id IS NOT DISTINCT FROM $3
                     AND expires_at > NOW()
               ) AS "bound!""#,
            session_id.as_str(),
            server.as_str(),
            principal.map(UserId::as_str),
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(bound)
    }

    pub async fn cleanup_expired(&self) -> McpDomainResult<u64> {
        let result = sqlx::query!("DELETE FROM mcp_external_sessions WHERE expires_at <= NOW()")
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }
}
