//! The two degenerate terminal signals, crossed with every outbound wire.
//!
//! The matrix in `gateway_matrix_*_in` drives a well-behaved upstream: one that
//! declares tool use when it emits a tool call. These cells drive the two
//! upstreams that do not, which is where the tool call is actually lost.
//!
//! `GenericStop` is the outage that shipped twice -- a fully-formed call under
//! a plain "stop"/"end_turn", which an OpenAI-contract client reads as a
//! finished turn before discarding the call. `Truncated` is its mirror: the
//! correction for the first must not swallow a real `max_tokens` cutoff, or a
//! client is handed a call whose arguments are incomplete JSON.
//!
//! Both run on the `OpenAI` Chat Completions inbound, whose `finish_reason` is
//! a single scalar and so distinguishes the two outcomes unambiguously.

use std::sync::Arc;

use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;

use super::gateway_matrix::{
    OutWire, Scenario, assert_declares_tool_use, assert_declares_truncation,
    assert_tool_call_survived, openai_chat_request_body, run_scenario,
};

const TOOL_USE_MARKER: &str = "\"finish_reason\":\"tool_calls\"";
const TRUNCATED_MARKER: &str = "\"finish_reason\":\"length\"";

async fn generic_stop(label: &str, out: OutWire, stream: bool) -> anyhow::Result<()> {
    let rendered = run_scenario(
        label,
        out,
        Scenario::GenericStop,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(stream),
        stream,
    )
    .await?;
    assert_tool_call_survived(label, &rendered);
    assert_declares_tool_use(label, &rendered, TOOL_USE_MARKER);
    Ok(())
}

async fn truncated(label: &str, out: OutWire, stream: bool) -> anyhow::Result<()> {
    let rendered = run_scenario(
        label,
        out,
        Scenario::Truncated,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(stream),
        stream,
    )
    .await?;
    assert_declares_truncation(label, &rendered, TRUNCATED_MARKER);
    Ok(())
}

#[tokio::test]
async fn anthropic_out_buffered_generic_stop_still_declares_tool_use() -> anyhow::Result<()> {
    generic_stop("degenerate-anthropic-buffered", OutWire::Anthropic, false).await
}

#[tokio::test]
async fn anthropic_out_streaming_generic_stop_still_declares_tool_use() -> anyhow::Result<()> {
    generic_stop("degenerate-anthropic-streaming", OutWire::Anthropic, true).await
}

#[tokio::test]
async fn gemini_out_buffered_generic_stop_still_declares_tool_use() -> anyhow::Result<()> {
    generic_stop("degenerate-gemini-buffered", OutWire::Gemini, false).await
}

#[tokio::test]
async fn gemini_out_streaming_generic_stop_still_declares_tool_use() -> anyhow::Result<()> {
    generic_stop("degenerate-gemini-streaming", OutWire::Gemini, true).await
}

#[tokio::test]
async fn openai_chat_out_buffered_generic_stop_still_declares_tool_use() -> anyhow::Result<()> {
    generic_stop(
        "degenerate-openai_chat-buffered",
        OutWire::OpenAiChat,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_out_streaming_generic_stop_still_declares_tool_use() -> anyhow::Result<()> {
    generic_stop(
        "degenerate-openai_chat-streaming",
        OutWire::OpenAiChat,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_out_buffered_generic_stop_still_declares_tool_use() -> anyhow::Result<()>
{
    generic_stop(
        "degenerate-openai_responses-buffered",
        OutWire::OpenAiResponses,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_out_streaming_generic_stop_still_declares_tool_use() -> anyhow::Result<()>
{
    generic_stop(
        "degenerate-openai_responses-streaming",
        OutWire::OpenAiResponses,
        true,
    )
    .await
}

#[tokio::test]
async fn anthropic_out_buffered_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated("truncated-anthropic-buffered", OutWire::Anthropic, false).await
}

#[tokio::test]
async fn anthropic_out_streaming_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated("truncated-anthropic-streaming", OutWire::Anthropic, true).await
}

#[tokio::test]
async fn gemini_out_buffered_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated("truncated-gemini-buffered", OutWire::Gemini, false).await
}

#[tokio::test]
async fn gemini_out_streaming_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated("truncated-gemini-streaming", OutWire::Gemini, true).await
}

#[tokio::test]
async fn openai_chat_out_buffered_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated("truncated-openai_chat-buffered", OutWire::OpenAiChat, false).await
}

#[tokio::test]
async fn openai_chat_out_streaming_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated("truncated-openai_chat-streaming", OutWire::OpenAiChat, true).await
}

#[tokio::test]
async fn openai_responses_out_buffered_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated(
        "truncated-openai_responses-buffered",
        OutWire::OpenAiResponses,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_out_streaming_truncated_call_reports_the_cutoff() -> anyhow::Result<()> {
    truncated(
        "truncated-openai_responses-streaming",
        OutWire::OpenAiResponses,
        true,
    )
    .await
}

// Why: the Anthropic inbound renders MessageStop through `render_event`, which
// the tap's repeat-stop guard did not cover -- it only gated the terminal
// render. An Anthropic upstream ends a stream twice (`message_delta` with the
// real reason, then a reason-less `message_stop` frame), so the client received
// a second `message_delta` saying `end_turn` after the real `tool_use` one, and
// an SDK that reads the last terminal frame dropped the call.
#[tokio::test]
async fn anthropic_in_anthropic_out_streaming_ends_the_turn_exactly_once() -> anyhow::Result<()> {
    use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;

    use super::gateway_matrix::anthropic_request_body;

    let label = "single-stop-anthropic-streaming";
    let rendered = run_scenario(
        label,
        OutWire::Anthropic,
        Scenario::ToolCall,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived(label, &rendered);
    assert_eq!(
        rendered.matches("event: message_stop").count(),
        1,
        "the turn must end exactly once; body: {rendered}"
    );
    assert!(
        !rendered.contains("\"stop_reason\":\"end_turn\""),
        "no weaker terminal reason may follow the tool_use one; body: {rendered}"
    );
    Ok(())
}
