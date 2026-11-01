//! The brain's MCP tools.
//!
//! [`BrainMcpTools`] is what an MCP server composes:
//! [`BrainMcpTools::list_tools`] for `tools/list` and [`BrainMcpTools::call`]
//! to run one tool through core's [`McpToolExecutor`] after the server has
//! authenticated the request. Memory reaches the model through the gateway on
//! its own; these tools inspect it, propose facts as `proposed` and dispute
//! them. Every call acts as the authenticated user, and its actor is always the
//! agent: a tool never confirms, retracts, pins or erases a fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod dispute;
mod guide;
mod propose;
mod refusal;
mod search;
mod tools;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use systemprompt_brain::service::{BrainCaller, BrainService};
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_mcp::{ClientProfile, McpToolExecutor, McpToolHandler};
use systemprompt_models::auth::{RequestActor, agent_name};
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};

pub use dispute::DisputeHandler;
pub use guide::{GuideHandler, guide_text, refusal_text};
pub use propose::ProposeHandler;
pub use refusal::{
    BrainRefusalCode, brain_tool_error, refusal, refusal_code, refusal_of, refusal_result,
};
pub use search::{RecallPreviewHandler, SearchHandler, WhyHandler};
pub use tools::{
    DisputeInput, DisputeOutput, FactItem, GuideInput, GuideOutput, ProposeInput, ProposeOutput,
    RecallPreviewInput, RecallPreviewOutput, SearchInput, SearchOutput, WhyInput, WhyOutput,
};

pub const TOOL_NAMES: [&str; 6] = [
    "brain_search",
    "brain_propose",
    "brain_dispute",
    "brain_why",
    "brain_recall_preview",
    "brain_guide",
];

const GOVERNING_TOOLS: [&str; 6] = [
    "brain_confirm",
    "brain_approve",
    "brain_retract",
    "brain_pin",
    "brain_erase",
    "brain_resolve",
];

/// Why [`BrainMcpTools::from_parts`] refused to build the tools.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum BrainMcpUnavailable {
    #[error("the brain MCP tools need the brain: set brain.enabled: true in the profile")]
    Disabled,
}

/// One tool call, already authenticated by the composing server.
#[derive(Debug, Clone, Copy)]
pub struct BrainToolCall<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub context: &'a RequestContext,
    pub client: &'a ClientProfile,
}

/// The brain tools of one MCP server.
#[derive(Clone)]
pub struct BrainMcpTools {
    service: Arc<BrainService>,
    recall: Arc<dyn MemoryRecall>,
    profile: Arc<BrainProfile>,
}

impl std::fmt::Debug for BrainMcpTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainMcpTools")
            .field("recall", &self.recall.name())
            .finish_non_exhaustive()
    }
}

impl BrainMcpTools {
    pub fn from_parts(
        service: Arc<BrainService>,
        recall: Arc<dyn MemoryRecall>,
        profile: Arc<BrainProfile>,
    ) -> Result<Self, BrainMcpUnavailable> {
        if !profile.enabled {
            return Err(BrainMcpUnavailable::Disabled);
        }
        Ok(Self {
            service,
            recall,
            profile,
        })
    }

    #[must_use]
    pub fn service(&self) -> &BrainService {
        &self.service
    }

    #[must_use]
    pub fn recall(&self) -> &dyn MemoryRecall {
        self.recall.as_ref()
    }

    #[must_use]
    pub fn profile(&self) -> &BrainProfile {
        &self.profile
    }

    #[must_use]
    pub const fn tool_names() -> &'static [&'static str] {
        &TOOL_NAMES
    }

    #[must_use]
    pub fn list_tools(&self, server: &McpServerId) -> Vec<Tool> {
        vec![
            tools::compact(SearchHandler { tools: self }.tool_definition(server)),
            tools::compact(ProposeHandler { tools: self }.tool_definition(server)),
            tools::compact(DisputeHandler { tools: self }.tool_definition(server)),
            tools::compact(WhyHandler { tools: self }.tool_definition(server)),
            tools::compact(RecallPreviewHandler { tools: self }.tool_definition(server)),
            tools::compact(GuideHandler.tool_definition(server)),
        ]
    }

    pub async fn call(&self, call: BrainToolCall<'_>) -> Result<CallToolResult, McpError> {
        match call.request.name.as_ref() {
            "brain_search" => Box::pin(run(call, &SearchHandler { tools: self })).await,
            "brain_propose" => Box::pin(run(call, &ProposeHandler { tools: self })).await,
            "brain_dispute" => Box::pin(run(call, &DisputeHandler { tools: self })).await,
            "brain_why" => Box::pin(run(call, &WhyHandler { tools: self })).await,
            "brain_recall_preview" => {
                Box::pin(run(call, &RecallPreviewHandler { tools: self })).await
            },
            "brain_guide" => Box::pin(run(call, &GuideHandler)).await,
            governing if GOVERNING_TOOLS.contains(&governing) => {
                refusal_result(&agent_forbidden()).ok_or_else(agent_forbidden)
            },
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown tool: '{other}'. This server exposes: {}.",
                    TOOL_NAMES.join(", ")
                ),
                None,
            )),
        }
    }
}

pub(super) fn agent_forbidden() -> McpError {
    refusal(
        BrainRefusalCode::AgentForbidden,
        "an agent proposes facts and never confirms, retracts, pins or erases them; a person \
         decides in the brain review",
    )
}

pub(super) fn brain_caller(ctx: &RequestContext) -> Result<BrainCaller, McpError> {
    let user = ctx.user.as_ref().ok_or_else(|| {
        refusal(
            BrainRefusalCode::Forbidden,
            "brain tools act for the signed-in user; this call carried no authenticated user",
        )
    })?;
    Ok(BrainCaller::new(user.id.clone(), user.roles.clone()).with_actor(agent_actor(ctx)))
}

pub(super) fn agent_actor(ctx: &RequestContext) -> RequestActor {
    let attended = ctx.settings.user_interaction_mode == Some(UserInteractionMode::Interactive)
        && ctx.caller_purpose().is_none();
    RequestActor::Agent {
        name: agent_name(ctx.caller_purpose(), ctx.client_id()),
        attended,
    }
}

async fn run<H: McpToolHandler>(
    call: BrainToolCall<'_>,
    handler: &H,
) -> Result<CallToolResult, McpError> {
    match call
        .executor
        .execute(handler, call.request, call.context, call.client)
        .await
    {
        Err(error) => refusal_result(&error).ok_or(error),
        ok => ok,
    }
}
