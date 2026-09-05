//! The two degenerate terminal signals, on every inbound surface, for the two
//! outbound wires that have no terminal file of their own.
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
//! Until this file was parameterised it hardcoded the Chat Completions
//! inbound, so the degenerate grid was one inbound against four outbound wires
//! -- a third of the happy path's coverage. The translation that computes the
//! terminal reason is per-inbound, so a surface that was never crossed with a
//! degenerate upstream was never proven at all. The grid is now closed across
//! three files with no cell run twice: `gateway_matrix_anthropic_terminal` owns
//! the Anthropic outbound wire, `gateway_matrix_openai_responses_terminal` owns
//! the Responses wire, and this file owns Gemini and Chat Completions.

use super::gateway_matrix::{OutWire, Scenario};
use super::gateway_matrix_inbound::{InWire, assert_declared_tool_use, assert_reported_truncation};

// Why: Gemini's `finishReason: STOP` on a functionCall candidate is the
// literal input that shipped the outage, and it is the same lie on every
// inbound surface -- only the vocabulary the caller reads it in differs.
#[tokio::test]
async fn anthropic_in_gemini_out_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "gemini-out-generic",
        OutWire::Gemini,
        InWire::Anthropic,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_gemini_out_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "gemini-out-generic",
        OutWire::Gemini,
        InWire::Anthropic,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_gemini_out_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "gemini-out-generic",
        OutWire::Gemini,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_gemini_out_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()>
{
    assert_declared_tool_use(
        "gemini-out-generic",
        OutWire::Gemini,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_gemini_out_buffered_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "gemini-out-generic",
        OutWire::Gemini,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_gemini_out_streaming_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "gemini-out-generic",
        OutWire::Gemini,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        true,
    )
    .await
}

// Why: several OpenAI-compatible fronts (Vertex MaaS, Cerebras, proxies) send a
// fully-formed `tool_calls` array under `finish_reason: "stop"`, so the Chat
// Completions wire carries the same degenerate shape Gemini does.
#[tokio::test]
async fn anthropic_in_openai_chat_out_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()>
{
    assert_declared_tool_use(
        "openai_chat-out-generic",
        OutWire::OpenAiChat,
        InWire::Anthropic,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_openai_chat_out_streaming_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "openai_chat-out-generic",
        OutWire::OpenAiChat,
        InWire::Anthropic,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_openai_chat_out_buffered_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "openai_chat-out-generic",
        OutWire::OpenAiChat,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_openai_chat_out_streaming_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "openai_chat-out-generic",
        OutWire::OpenAiChat,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_openai_chat_out_buffered_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "openai_chat-out-generic",
        OutWire::OpenAiChat,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_openai_chat_out_streaming_generic_stop_declares_tool_use()
-> anyhow::Result<()> {
    assert_declared_tool_use(
        "openai_chat-out-generic",
        OutWire::OpenAiChat,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_gemini_out_buffered_truncated_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "gemini-out-truncated",
        OutWire::Gemini,
        InWire::Anthropic,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_gemini_out_streaming_truncated_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "gemini-out-truncated",
        OutWire::Gemini,
        InWire::Anthropic,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_gemini_out_buffered_truncated_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "gemini-out-truncated",
        OutWire::Gemini,
        InWire::OpenAiChat,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_gemini_out_streaming_truncated_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "gemini-out-truncated",
        OutWire::Gemini,
        InWire::OpenAiChat,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_gemini_out_buffered_truncated_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "gemini-out-truncated",
        OutWire::Gemini,
        InWire::OpenAiResponses,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_gemini_out_streaming_truncated_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "gemini-out-truncated",
        OutWire::Gemini,
        InWire::OpenAiResponses,
        true,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_openai_chat_out_buffered_truncated_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "openai_chat-out-truncated",
        OutWire::OpenAiChat,
        InWire::Anthropic,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_openai_chat_out_streaming_truncated_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "openai_chat-out-truncated",
        OutWire::OpenAiChat,
        InWire::Anthropic,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_openai_chat_out_buffered_truncated_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "openai_chat-out-truncated",
        OutWire::OpenAiChat,
        InWire::OpenAiChat,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_openai_chat_out_streaming_truncated_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "openai_chat-out-truncated",
        OutWire::OpenAiChat,
        InWire::OpenAiChat,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_openai_chat_out_buffered_truncated_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "openai_chat-out-truncated",
        OutWire::OpenAiChat,
        InWire::OpenAiResponses,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_openai_chat_out_streaming_truncated_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "openai_chat-out-truncated",
        OutWire::OpenAiChat,
        InWire::OpenAiResponses,
        true,
    )
    .await
}
