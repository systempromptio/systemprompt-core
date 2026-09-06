//! The OpenAI Chat Completions inbound surface crossed with every outbound
//! wire.
//!
//! `finish_reason: "tool_calls"` is the field OpenCode, Copilot BYOK and every
//! other OpenAI-SDK client reads; `"stop"` beside a tool_calls array ends the
//! turn and drops the call.
//!
//! Every test here is one matrix cell: a real caller body in this dialect,
//! translated out to the provider's wire, answered with a tool call in that
//! wire's own shape, and translated back. See `gateway_matrix` for the harness
//! and for why the terminal-reason assertion is the load-bearing one.

use std::sync::Arc;

use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;

use super::gateway_matrix::{
    OutWire, assert_declares_tool_use, assert_tool_call_survived, openai_chat_request_body,
    run_cell,
};

const TERMINAL_MARKER: &str = "\"finish_reason\":\"tool_calls\"";

#[tokio::test]
async fn openai_chat_in_anthropic_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_chat-anthropic-buffered",
        OutWire::Anthropic,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_chat-anthropic-buffered", &rendered);
    assert_declares_tool_use("openai_chat-anthropic-buffered", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_anthropic_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_chat-anthropic-streaming",
        OutWire::Anthropic,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_chat-anthropic-streaming", &rendered);
    assert_declares_tool_use(
        "openai_chat-anthropic-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_gemini_out_buffered_keeps_the_tool_call_and_says_so() -> anyhow::Result<()>
{
    let rendered = run_cell(
        "openai_chat-gemini-buffered",
        OutWire::Gemini,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_chat-gemini-buffered", &rendered);
    assert_declares_tool_use("openai_chat-gemini-buffered", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_gemini_out_streaming_keeps_the_tool_call_and_says_so() -> anyhow::Result<()>
{
    let rendered = run_cell(
        "openai_chat-gemini-streaming",
        OutWire::Gemini,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_chat-gemini-streaming", &rendered);
    assert_declares_tool_use("openai_chat-gemini-streaming", &rendered, TERMINAL_MARKER);
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_openai_chat_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_chat-openai_chat-buffered",
        OutWire::OpenAiChat,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_chat-openai_chat-buffered", &rendered);
    assert_declares_tool_use(
        "openai_chat-openai_chat-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_openai_chat_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_chat-openai_chat-streaming",
        OutWire::OpenAiChat,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_chat-openai_chat-streaming", &rendered);
    assert_declares_tool_use(
        "openai_chat-openai_chat-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_openai_responses_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_chat-openai_responses-buffered",
        OutWire::OpenAiResponses,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_chat-openai_responses-buffered", &rendered);
    assert_declares_tool_use(
        "openai_chat-openai_responses-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_chat_in_openai_responses_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_chat-openai_responses-streaming",
        OutWire::OpenAiResponses,
        Arc::new(OpenAiChatInbound),
        openai_chat_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_chat-openai_responses-streaming", &rendered);
    assert_declares_tool_use(
        "openai_chat-openai_responses-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}
