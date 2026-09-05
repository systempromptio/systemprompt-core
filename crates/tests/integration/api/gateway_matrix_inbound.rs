//! The inbound half of a matrix cell, as data.
//!
//! `gateway_matrix` names the four outbound wires; this names the three
//! inbound surfaces alongside the two strings that decide whether a cell
//! passed. Terminal reasons are the whole point of the matrix and each surface
//! spells them differently -- Anthropic reads `stop_reason`, Chat Completions
//! reads `finish_reason`, and the Responses surface reads `stop_reason` with
//! `OpenAI`'s vocabulary in it. Hard-coding those markers per test file is how
//! a cell ends up asserting a marker its own surface never emits, which always
//! passes.

use std::sync::Arc;

use bytes::Bytes;
use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;

use super::gateway_matrix::{
    OutWire, Scenario, anthropic_request_body, assert_declares_tool_use,
    assert_declares_truncation, assert_no_complete_tool_use, assert_tool_call_survived,
    openai_chat_request_body, openai_responses_request_body, run_scenario,
};

/// The caller-facing surface of one matrix cell.
#[derive(Debug, Clone, Copy)]
pub(super) enum InWire {
    Anthropic,
    OpenAiChat,
    OpenAiResponses,
}

impl InWire {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAiChat => "openai_chat",
            Self::OpenAiResponses => "openai_responses",
        }
    }

    pub(super) fn adapter(self) -> Arc<dyn InboundAdapter> {
        match self {
            Self::Anthropic => Arc::new(AnthropicMessagesInbound),
            Self::OpenAiChat => Arc::new(OpenAiChatInbound),
            Self::OpenAiResponses => Arc::new(OpenAiResponsesInbound),
        }
    }

    pub(super) fn request_body(self, stream: bool) -> Bytes {
        match self {
            Self::Anthropic => anthropic_request_body(stream),
            Self::OpenAiChat => openai_chat_request_body(stream),
            Self::OpenAiResponses => openai_responses_request_body(stream),
        }
    }

    // Why: the marker a conforming client reads to decide it must run the tool.
    pub(super) const fn tool_use_marker(self) -> &'static str {
        match self {
            Self::Anthropic => "\"stop_reason\":\"tool_use\"",
            Self::OpenAiChat => "\"finish_reason\":\"tool_calls\"",
            Self::OpenAiResponses => "\"status\":\"completed\"",
        }
    }

    // Why: Anthropic keeps its own vocabulary (`max_tokens`); both OpenAI
    // surfaces map the same canonical reason to `length`.
    pub(super) const fn truncation_marker(self) -> &'static str {
        match self {
            Self::Anthropic => "\"stop_reason\":\"max_tokens\"",
            Self::OpenAiChat => "\"finish_reason\":\"length\"",
            Self::OpenAiResponses => "\"reason\":\"max_output_tokens\"",
        }
    }
}

// Why: every terminal cell asserts the same two things about the same
// rendered body, and the pair differs only in which marker is the one that
// must be present and which must be absent.
pub(super) async fn assert_declared_tool_use(
    prefix: &str,
    out: OutWire,
    inbound: InWire,
    scenario: Scenario,
    stream: bool,
) -> anyhow::Result<()> {
    let label = cell_label(prefix, inbound, stream);
    let rendered = run_scenario(
        &label,
        out,
        scenario,
        inbound.adapter(),
        inbound.request_body(stream),
        stream,
    )
    .await?;
    assert_tool_call_survived(&label, &rendered);
    assert_declares_tool_use(&label, &rendered, inbound.tool_use_marker());
    Ok(())
}

pub(super) async fn assert_reported_truncation(
    prefix: &str,
    out: OutWire,
    inbound: InWire,
    stream: bool,
) -> anyhow::Result<()> {
    let label = cell_label(prefix, inbound, stream);
    let rendered = run_scenario(
        &label,
        out,
        Scenario::Truncated,
        inbound.adapter(),
        inbound.request_body(stream),
        stream,
    )
    .await?;
    assert_declares_truncation(&label, &rendered, inbound.truncation_marker());
    assert_no_complete_tool_use(&label, &rendered, inbound.tool_use_marker());
    Ok(())
}

fn cell_label(prefix: &str, inbound: InWire, stream: bool) -> String {
    let lane = if stream { "streaming" } else { "buffered" };
    format!("{prefix}-{}-{lane}", inbound.label())
}
