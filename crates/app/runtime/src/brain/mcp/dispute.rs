//! An agent flags a fact it doubts rather than changing it: the people who
//! govern the fact are notified and decide.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::refusal::{BrainRefusalCode, brain_tool_error, refusal};
use super::tools::{DisputeInput, DisputeOutput};
use super::{BrainMcpTools, brain_caller};

const MAX_REASON_CHARS: usize = 2000;

#[derive(Debug, Clone, Copy)]
pub struct DisputeHandler<'a> {
    pub tools: &'a BrainMcpTools,
}

impl McpToolHandler for DisputeHandler<'_> {
    type Input = DisputeInput;
    type Output = DisputeOutput;

    fn tool_name(&self) -> &'static str {
        "brain_dispute"
    }

    fn description(&self) -> &'static str {
        "Dispute a fact, with a reason."
    }

    async fn handle(
        &self,
        input: DisputeInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(DisputeOutput, String), McpError> {
        let reason = input.reason.trim();
        if reason.is_empty() || reason.chars().count() > MAX_REASON_CHARS {
            return Err(refusal(
                BrainRefusalCode::InvalidInput,
                format!("reason must be 1 to {MAX_REASON_CHARS} characters"),
            ));
        }
        let caller = brain_caller(ctx)?;
        let dispute_id = self
            .tools
            .service()
            .dispute(&caller, &input.fact_id, reason)
            .await
            .map_err(brain_tool_error)?;
        let summary = format!("disputed {}", input.fact_id);
        Ok((DisputeOutput { dispute_id }, summary))
    }
}
