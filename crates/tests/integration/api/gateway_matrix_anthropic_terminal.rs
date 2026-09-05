//! The Anthropic outbound wire's terminal signal, on every inbound surface.
//!
//! `gateway_matrix_degenerate` drives the two degenerate upstreams through the
//! Chat Completions inbound only, so the Anthropic provider's own terminal
//! frame was proven for exactly one of the three surfaces a caller can use.
//! The translation that computes the terminal reason is per-inbound -- each
//! surface renders it from the canonical reason in its own vocabulary -- so a
//! surface that was never crossed with this wire was never proven at all.
//!
//! Two upstreams, both real. `GenericStop` is an Anthropic reply carrying a
//! well-formed `tool_use` block under `stop_reason: "end_turn"`; the turn must
//! still be declared as tool use or the client ends it and drops the call.
//! `Truncated` is `max_tokens` with the tool arguments cut mid-JSON; the
//! cutoff must be what the caller sees, and the partial call must not also be
//! announced as a complete one.

use super::gateway_matrix::{OutWire, Scenario};
use super::gateway_matrix_inbound::{InWire, assert_declared_tool_use, assert_reported_truncation};

// Why: same-wire Anthropic in to Anthropic out is byte passthrough, so the
// canonical `with_tool_use` correction is never reached on this path. The lane
// stays byte-faithful and rewrites only the one contradictory token -- see
// `protocol/outbound/anthropic/terminal.rs`. Anthropic itself never sends
// `end_turn` beside a `tool_use` block, so the exposure this closes is
// non-Anthropic upstreams speaking the Anthropic wire (Bedrock, Vertex,
// proxies), where it was a dropped tool call.
#[tokio::test]
async fn anthropic_in_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "anthropic-out-generic",
        OutWire::Anthropic,
        InWire::Anthropic,
        Scenario::GenericStop,
        false,
    )
    .await
}

// Why: same-wire Anthropic in to Anthropic out is byte passthrough, so the
// canonical `with_tool_use` correction is never reached on this path. The lane
// stays byte-faithful and rewrites only the one contradictory token -- see
// `protocol/outbound/anthropic/terminal.rs`. Anthropic itself never sends
// `end_turn` beside a `tool_use` block, so the exposure this closes is
// non-Anthropic upstreams speaking the Anthropic wire (Bedrock, Vertex,
// proxies), where it was a dropped tool call.
#[tokio::test]
async fn anthropic_in_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "anthropic-out-generic",
        OutWire::Anthropic,
        InWire::Anthropic,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "anthropic-out-generic",
        OutWire::Anthropic,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "anthropic-out-generic",
        OutWire::Anthropic,
        InWire::OpenAiChat,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_buffered_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "anthropic-out-generic",
        OutWire::Anthropic,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_streaming_generic_stop_declares_tool_use() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "anthropic-out-generic",
        OutWire::Anthropic,
        InWire::OpenAiResponses,
        Scenario::GenericStop,
        true,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_buffered_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "anthropic-out-truncated",
        OutWire::Anthropic,
        InWire::Anthropic,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_streaming_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()> {
    assert_reported_truncation(
        "anthropic-out-truncated",
        OutWire::Anthropic,
        InWire::Anthropic,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_buffered_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "anthropic-out-truncated",
        OutWire::Anthropic,
        InWire::OpenAiChat,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_streaming_truncated_mid_tool_call_reports_the_cutoff() -> anyhow::Result<()>
{
    assert_reported_truncation(
        "anthropic-out-truncated",
        OutWire::Anthropic,
        InWire::OpenAiChat,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_buffered_truncated_mid_tool_call_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "anthropic-out-truncated",
        OutWire::Anthropic,
        InWire::OpenAiResponses,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_streaming_truncated_mid_tool_call_reports_the_cutoff()
-> anyhow::Result<()> {
    assert_reported_truncation(
        "anthropic-out-truncated",
        OutWire::Anthropic,
        InWire::OpenAiResponses,
        true,
    )
    .await
}
