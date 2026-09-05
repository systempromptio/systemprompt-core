//! The OpenAI Responses inbound surface crossed with every outbound wire.
//!
//! Codex finalizes a call from the `response.completed` frame and its
//! `stop_reason`; a `"stop"` there strands the call.
//!
//! Every test here is one matrix cell: a real caller body in this dialect,
//! translated out to the provider's wire, answered with a tool call in that
//! wire's own shape, and translated back. See `gateway_matrix` for the harness
//! and for why the terminal-reason assertion is the load-bearing one.

use std::sync::Arc;

use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;

use super::gateway_matrix::{
    OutWire, assert_declares_tool_use, assert_tool_call_survived, openai_responses_request_body,
    run_cell,
};

const TERMINAL_MARKER: &str = "\"stop_reason\":\"tool_calls\"";

#[tokio::test]
async fn openai_responses_in_anthropic_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-anthropic-buffered",
        OutWire::Anthropic,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_responses-anthropic-buffered", &rendered);
    assert_declares_tool_use(
        "openai_responses-anthropic-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_anthropic_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-anthropic-streaming",
        OutWire::Anthropic,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_responses-anthropic-streaming", &rendered);
    assert_declares_tool_use(
        "openai_responses-anthropic-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_gemini_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-gemini-buffered",
        OutWire::Gemini,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_responses-gemini-buffered", &rendered);
    assert_declares_tool_use(
        "openai_responses-gemini-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_gemini_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-gemini-streaming",
        OutWire::Gemini,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_responses-gemini-streaming", &rendered);
    assert_declares_tool_use(
        "openai_responses-gemini-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_openai_chat_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-openai_chat-buffered",
        OutWire::OpenAiChat,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_responses-openai_chat-buffered", &rendered);
    assert_declares_tool_use(
        "openai_responses-openai_chat-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_openai_chat_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-openai_chat-streaming",
        OutWire::OpenAiChat,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_responses-openai_chat-streaming", &rendered);
    assert_declares_tool_use(
        "openai_responses-openai_chat-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_openai_responses_out_buffered_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-openai_responses-buffered",
        OutWire::OpenAiResponses,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(false),
        false,
    )
    .await?;
    assert_tool_call_survived("openai_responses-openai_responses-buffered", &rendered);
    assert_declares_tool_use(
        "openai_responses-openai_responses-buffered",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}

#[tokio::test]
async fn openai_responses_in_openai_responses_out_streaming_keeps_the_tool_call_and_says_so()
-> anyhow::Result<()> {
    let rendered = run_cell(
        "openai_responses-openai_responses-streaming",
        OutWire::OpenAiResponses,
        Arc::new(OpenAiResponsesInbound),
        openai_responses_request_body(true),
        true,
    )
    .await?;
    assert_tool_call_survived("openai_responses-openai_responses-streaming", &rendered);
    assert_declares_tool_use(
        "openai_responses-openai_responses-streaming",
        &rendered,
        TERMINAL_MARKER,
    );
    Ok(())
}
