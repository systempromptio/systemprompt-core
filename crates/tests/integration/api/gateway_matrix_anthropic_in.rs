//! The Anthropic Messages inbound surface crossed with every outbound wire.
//!
//! `stop_reason: "tool_use"` is the only field an Anthropic-contract client
//! reads to decide whether to run the tool.
//!
//! Every test here is one matrix cell: a real caller body in this dialect,
//! translated out to the provider's wire, answered with a tool call in that
//! wire's own shape, and translated back. See `gateway_matrix` for the harness
//! and for why the terminal-reason assertion is the load-bearing one.

use std::sync::Arc;

use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;

use super::gateway_matrix::{
    OutWire, anthropic_request_body, assert_declares_tool_use, assert_tool_call_survived, run_cell,
};

const TERMINAL_MARKER: &str = "\"stop_reason\":\"tool_use\"";

#[tokio::test]
async fn anthropic_in_anthropic_out_buffered_keeps_the_tool_call_and_says_so() -> anyhow::Result<()>
{
    let rendered = run_cell(
        "anthropic-anthropic-buffered",
        OutWire::Anthropic,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("anthropic-anthropic-buffered", &rendered);
    assert_declares_tool_use("anthropic-anthropic-buffered", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn anthropic_in_anthropic_out_streaming_keeps_the_tool_call_and_says_so() -> anyhow::Result<()>
{
    let rendered = run_cell(
        "anthropic-anthropic-streaming",
        OutWire::Anthropic,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("anthropic-anthropic-streaming", &rendered);
    assert_declares_tool_use("anthropic-anthropic-streaming", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn anthropic_in_gemini_out_buffered_keeps_the_tool_call_and_says_so() -> anyhow::Result<()> {
    let rendered = run_cell(
        "anthropic-gemini-buffered",
        OutWire::Gemini,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("anthropic-gemini-buffered", &rendered);
    assert_declares_tool_use("anthropic-gemini-buffered", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn anthropic_in_gemini_out_streaming_keeps_the_tool_call_and_says_so() -> anyhow::Result<()> {
    let rendered = run_cell(
        "anthropic-gemini-streaming",
        OutWire::Gemini,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("anthropic-gemini-streaming", &rendered);
    assert_declares_tool_use("anthropic-gemini-streaming", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn anthropic_in_openai_chat_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "anthropic-openai_chat-buffered",
        OutWire::OpenAiChat,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("anthropic-openai_chat-buffered", &rendered);
    assert_declares_tool_use("anthropic-openai_chat-buffered", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn anthropic_in_openai_chat_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "anthropic-openai_chat-streaming",
        OutWire::OpenAiChat,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("anthropic-openai_chat-streaming", &rendered);
    assert_declares_tool_use(
        "anthropic-openai_chat-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn anthropic_in_openai_responses_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "anthropic-openai_responses-buffered",
        OutWire::OpenAiResponses,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("anthropic-openai_responses-buffered", &rendered);
    assert_declares_tool_use(
        "anthropic-openai_responses-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn anthropic_in_openai_responses_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "anthropic-openai_responses-streaming",
        OutWire::OpenAiResponses,
        Arc::new(AnthropicMessagesInbound),
        anthropic_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("anthropic-openai_responses-streaming", &rendered);
    assert_declares_tool_use(
        "anthropic-openai_responses-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}
