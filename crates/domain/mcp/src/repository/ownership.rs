//! [`OwnerReassignment`] for the mcp domain: moves every tool execution,
//! artifact and MCP session from one user to another in a single
//! transaction. Session-scoped identity caches (proxy identities, external
//! session bindings) are credentials minted for the old user and are
//! deleted rather than rebound.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{OwnerReassignment, ReassignedRows, RepositoryError};

use crate::error::McpDomainResult;

#[derive(Debug, Clone)]
pub struct McpOwnerReassignment {
    write_pool: Arc<PgPool>,
}

impl McpOwnerReassignment {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| crate::error::McpDomainError::Configuration(e.to_string()))?;
        Ok(Self { write_pool })
    }
}

#[async_trait]
impl OwnerReassignment for McpOwnerReassignment {
    fn domain(&self) -> &'static str {
        "mcp"
    }

    async fn reassign_owner(
        &self,
        from: &UserId,
        to: &UserId,
    ) -> Result<ReassignedRows, RepositoryError> {
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;

        let executions = sqlx::query!(
            "UPDATE mcp_tool_executions SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        let artifacts = sqlx::query!(
            "UPDATE mcp_artifacts SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        let sessions = sqlx::query!(
            "UPDATE mcp_sessions SET user_id = $2 WHERE user_id = $1",
            from.as_str(),
            to.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        let proxy_identities = sqlx::query!(
            "DELETE FROM mcp_proxy_identities WHERE user_id = $1",
            from.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        let external_sessions = sqlx::query!(
            "DELETE FROM mcp_external_sessions WHERE user_id = $1",
            from.as_str()
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected();

        tx.commit().await.map_err(RepositoryError::database)?;

        Ok(ReassignedRows {
            tables: vec![
                ("mcp_tool_executions", executions),
                ("mcp_artifacts", artifacts),
                ("mcp_sessions", sessions),
                ("mcp_proxy_identities", proxy_identities),
                ("mcp_external_sessions", external_sessions),
            ],
        })
    }
}
