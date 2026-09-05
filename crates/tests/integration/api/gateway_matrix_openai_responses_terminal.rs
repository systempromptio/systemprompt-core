//! The `OpenAI` Responses outbound wire's terminal signal, on every inbound
//! surface.
//!
//! The Responses dialect is the odd one: it has no finish-reason field at all.
//! A finished turn is a `response.completed` event, and a truncated one is
//! `response.incomplete` with `incomplete_details.reason`, so the canonical
//! terminal reason has to be reconstructed from the frame's identity rather
//! than read off a scalar. That reconstruction is the step no cell covered for
//! this wire outside the Chat Completions inbound.
//!
//! Two upstreams. `GenericStop` is a `completed` response whose output holds a
//! `function_call` item -- tool use must be declared regardless of the
//! terminal event's name. `Truncated` is the `incomplete` response with
//! `max_output_tokens`, whose arguments stopped mid-JSON.

use super::gateway_matrix::{OutWire, Scenario, run_scenario};
use super::gateway_matrix_inbound::{InWire, assert_declared_tool_use, assert_reported_truncation};

#[tokio::test]
async fn anthropic_in_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "responses-out-generic",
        OutWire::OpenAiResponses,
        InWire::Anthropic,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "responses-out-generic",
        OutWire::OpenAiResponses,
        InWire::Anthropic,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "responses-out-generic",
        OutWire::OpenAiResponses,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "responses-out-generic",
        OutWire::OpenAiResponses,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "responses-out-generic",
        OutWire::OpenAiResponses,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "responses-out-generic",
        OutWire::OpenAiResponses,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_buffered_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "responses-out-truncated",
        OutWire::OpenAiResponses,
        InWire::Anthropic,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_streaming_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "responses-out-truncated",
        OutWire::OpenAiResponses,
        InWire::Anthropic,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_buffered_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "responses-out-truncated",
        OutWire::OpenAiResponses,
        InWire::OpenAiChat,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_streaming_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "responses-out-truncated",
        OutWire::OpenAiResponses,
        InWire::OpenAiChat,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_buffered_truncated_mid_tool_call_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "responses-out-truncated",
        OutWire::OpenAiResponses,
        InWire::OpenAiResponses,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_streaming_truncated_mid_tool_call_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "responses-out-truncated",
        OutWire::OpenAiResponses,
        InWire::OpenAiResponses,
        true,
    )
    .await
}

// Why: a Codex-style client reads `status` and `incomplete_details`, not the
// `stop_reason` the truncation assertions above look for, so those cells prove
// the reason reached the body without proving the client can see it. The
// buffered lane must reach the same pair as the streaming terminal frame:
// `status: "incomplete"` with an `incomplete_details.reason` of
// `max_output_tokens`.
#[tokio::test]
async fn openai_responses_in_buffered_truncation_marks_the_response_incomplete()
-> anyhow::Result<()> {
    let label = "responses-out-truncated-status-buffered";
    let inbound = InWire::OpenAiResponses;
    let rendered = run_scenario(
        label,
        OutWire::OpenAiResponses,
        Scenario::Truncated,
        inbound.adapter(),
        inbound.request_body(false),
        false,
    )
    .await?;
    assert!(
        rendered.contains("\"status\":\"incomplete\"") && rendered.contains("max_output_tokens"),
        "{label}: a truncated buffered response must carry the incomplete status a Responses \
         client reads; body: {rendered}"
    );
    Ok(())
}
