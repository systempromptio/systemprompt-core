//! An OpenAI-compatible upstream that serializes its optional fields as
//! explicit `null`, crossed with every inbound surface.
//!
//! Vertex `MaaS` and most other OpenAI-compatible fronts do not omit the
//! optional keys the way `OpenAI` itself does. The assistant message arrives
//! with `content`, `refusal`, `function_call`, `annotations` and `audio` all
//! present and all `null`, beside a fully-formed `tool_calls` array, under a
//! plain `finish_reason: "stop"`.
//!
//! Every other Chat Completions fixture in the matrix omits those keys, so a
//! parser that reads "present but null" as malformed -- or as content -- looks
//! correct across the whole suite. These cells are the same tool-call
//! assertion as the rest of the matrix, driven by the body those providers
//! actually send: the call must survive the nulls, and the terminal reason
//! must still be corrected from the generic `"stop"` to tool use.

use super::gateway_matrix::{OutWire, Scenario};
use super::gateway_matrix_inbound::{InWire, assert_declared_tool_use};

#[tokio::test]
async fn anthropic_in_buffered_survives_explicit_null_optionals() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "nullfields",
        OutWire::OpenAiChat,
        InWire::Anthropic,
        Scenario::NullOptionalFields,
        false,
    )
    .await
}

#[tokio::test]
async fn anthropic_in_streaming_survives_explicit_null_optionals() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "nullfields",
        OutWire::OpenAiChat,
        InWire::Anthropic,
        Scenario::NullOptionalFields,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_buffered_survives_explicit_null_optionals() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "nullfields",
        OutWire::OpenAiChat,
        InWire::OpenAiChat,
        Scenario::NullOptionalFields,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_chat_in_streaming_survives_explicit_null_optionals() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "nullfields",
        OutWire::OpenAiChat,
        InWire::OpenAiChat,
        Scenario::NullOptionalFields,
        true,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_buffered_survives_explicit_null_optionals() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "nullfields",
        OutWire::OpenAiChat,
        InWire::OpenAiResponses,
        Scenario::NullOptionalFields,
        false,
    )
    .await
}

#[tokio::test]
async fn openai_responses_in_streaming_survives_explicit_null_optionals() -> anyhow::Result<()> {
    assert_declared_tool_use(
        "nullfields",
        OutWire::OpenAiChat,
        InWire::OpenAiResponses,
        Scenario::NullOptionalFields,
        true,
    )
    .await
}
