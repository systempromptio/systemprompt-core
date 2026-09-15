//! Read seam over the MCP tool-execution ledger.
//!
//! The `mcp_tool_executions` table is owned by the mcp domain; agent-side
//! artifact publishing needs to know whether an execution id it was handed is
//! real before recording it. The lookup is implemented by mcp and injected as
//! `Arc<dyn ToolExecutionLookup>`, so `#[async_trait]` is required for `dyn`
//! dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::McpExecutionId;

use crate::repository::RepositoryError;

/// Answers whether an MCP execution id exists in the owning domain's ledger.
///
/// Held as `Arc<dyn ToolExecutionLookup>` so the agent domain can be handed
/// whichever ledger the composition root wires in, hence `#[async_trait]`
/// for `dyn` dispatch. A lookup failure is an error, never `false`: the caller
/// must not treat an unreachable ledger as "unknown execution".
#[async_trait]
pub trait ToolExecutionLookup: Send + Sync {
    async fn execution_exists(&self, id: &McpExecutionId) -> Result<bool, RepositoryError>;
}

pub type DynToolExecutionLookup = Arc<dyn ToolExecutionLookup>;
