//! An agent enters a fact as `proposed` for a person to confirm. A personal
//! scope is the default; any other status than `proposed` is refused as
//! `agent_forbidden`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_brain::BrainError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::brain::{EnteredFact, EnteredStatus, FactScope, FactStatus, SubjectKey};
use systemprompt_models::execution::context::RequestContext;

use super::refusal::brain_tool_error;
use super::tools::{ProposeInput, ProposeOutput};
use super::{BrainMcpTools, agent_actor, agent_forbidden, brain_caller};

#[derive(Debug, Clone, Copy)]
pub struct ProposeHandler<'a> {
    pub tools: &'a BrainMcpTools,
}

impl McpToolHandler for ProposeHandler<'_> {
    type Input = ProposeInput;
    type Output = ProposeOutput;

    fn tool_name(&self) -> &'static str {
        "brain_propose"
    }

    fn description(&self) -> &'static str {
        "Propose a fact for a person to confirm. Guide: proposing."
    }

    async fn handle(
        &self,
        input: ProposeInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ProposeOutput, String), McpError> {
        if input
            .status
            .is_some_and(|status| status != FactStatus::Proposed)
        {
            return Err(agent_forbidden());
        }
        let caller = brain_caller(ctx)?;
        let subject: SubjectKey = input
            .subject
            .parse()
            .map_err(|error| brain_tool_error(BrainError::Subject(error)))?;
        let fact = EnteredFact {
            kind: input.kind,
            subject,
            fields: input.fields,
            scope: input
                .scope
                .unwrap_or_else(|| FactScope::User(caller.user_id.clone())),
            topics: input.topics.unwrap_or_default(),
            status: EnteredStatus::Proposed,
        };
        let outbox_id = self
            .tools
            .service()
            .enter(&caller, &fact)
            .await
            .map_err(brain_tool_error)?;
        tracing::info!(
            actor = ?agent_actor(ctx),
            kind = %fact.kind,
            outbox_id = %outbox_id,
            "Brain fact proposed over MCP"
        );
        let summary = format!("proposed {outbox_id}");
        Ok((ProposeOutput { outbox_id }, summary))
    }
}
