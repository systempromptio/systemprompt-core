//! Provider sessions bound to caller identity and credential on the primary
//! database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{SessionId, UserId};

use super::McpProxyIdentityRepository;
use crate::error::McpDomainResult;

#[derive(Debug)]
pub struct ExternalSessionBinding<'a> {
    pub server: &'a str,
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub credential_hash: &'a [u8],
}

impl McpProxyIdentityRepository {
    pub async fn accepts_external(
        &self,
        binding: &ExternalSessionBinding<'_>,
    ) -> McpDomainResult<bool> {
        let result = sqlx::query!(
            r#"UPDATE mcp_external_sessions
               SET expires_at = NOW() + INTERVAL '1 hour'
               WHERE server_name = $1 AND session_id = $2 AND user_id = $3
                 AND credential_hash = $4 AND expires_at > NOW()"#,
            binding.server,
            binding.session_id.as_str(),
            binding.user_id.as_str(),
            binding.credential_hash,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn remember_external(
        &self,
        binding: &ExternalSessionBinding<'_>,
    ) -> McpDomainResult<bool> {
        let result = sqlx::query!(
            r#"INSERT INTO mcp_external_sessions (server_name, session_id, user_id, credential_hash)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (server_name, session_id) DO UPDATE
               SET expires_at = NOW() + INTERVAL '1 hour',
                   user_id = EXCLUDED.user_id, credential_hash = EXCLUDED.credential_hash
               WHERE mcp_external_sessions.expires_at <= NOW()
                  OR (mcp_external_sessions.user_id = EXCLUDED.user_id
                      AND mcp_external_sessions.credential_hash = EXCLUDED.credential_hash)"#,
            binding.server,
            binding.session_id.as_str(),
            binding.user_id.as_str(),
            binding.credential_hash,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn forget_external(
        &self,
        binding: &ExternalSessionBinding<'_>,
    ) -> McpDomainResult<()> {
        sqlx::query!(
            r#"DELETE FROM mcp_external_sessions
               WHERE server_name = $1 AND session_id = $2 AND user_id = $3 AND credential_hash = $4"#,
            binding.server, binding.session_id.as_str(), binding.user_id.as_str(), binding.credential_hash,
        ).execute(&*self.write_pool).await?;
        Ok(())
    }
}
