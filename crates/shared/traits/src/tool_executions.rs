//! Seams between the MCP tool-execution ledger and the model's tool-call
//! intents.
//!
//! The `mcp_tool_executions` table is owned by the mcp domain; agent-side
//! artifact publishing needs to know whether an execution id it was handed is
//! real before recording it. The lookup is implemented by mcp and injected as
//! `Arc<dyn ToolExecutionLookup>`, so `#[async_trait]` is required for `dyn`
//! dispatch.
//!
//! The `ai_request_tool_calls` intents are owned by the ai domain; an MCP
//! execution claims the intent it fulfils through [`ToolCallIntentClaims`],
//! implemented by ai and injected as `Arc<dyn ToolCallIntentClaims>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::{AiToolCallId, McpExecutionId, SessionId};

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

/// Claims a model's tool-call intent for the MCP execution that fulfils it.
///
/// Held as `Arc<dyn ToolCallIntentClaims>` so the mcp domain can be handed
/// whichever intent store the composition root wires in, hence
/// `#[async_trait]` for `dyn` dispatch. An intent is claimed at most once: a
/// claim only stamps an intent no execution holds yet, and a lost race is
/// `None`/`false`, never an overwrite. The execution row must exist before a
/// claim names it.
#[async_trait]
pub trait ToolCallIntentClaims: Send + Sync {
    async fn claim_newest_unclaimed(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        execution: &McpExecutionId,
        window_seconds: i64,
    ) -> Result<Option<AiToolCallId>, RepositoryError>;

    async fn claim(
        &self,
        call: &AiToolCallId,
        execution: &McpExecutionId,
    ) -> Result<bool, RepositoryError>;

    async fn release(
        &self,
        call: &AiToolCallId,
        execution: &McpExecutionId,
    ) -> Result<bool, RepositoryError>;
}

pub type DynToolCallIntentClaims = Arc<dyn ToolCallIntentClaims>;
