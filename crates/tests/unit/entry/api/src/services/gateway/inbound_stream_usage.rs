//! Where a streamed turn states its token counts, on each inbound surface.
//!
//! Chat Completions reports usage in a trailing chunk of its own, after the
//! finish chunk and before `[DONE]`, and only when the caller asked for it
//! with `stream_options.include_usage`. The gateway used to put a `usage`
//! object on the finish chunk instead -- rendered before the upstream's own
//! usage chunk had arrived, so every count in it was zero and the real ones,
//! which the audit row recorded, never reached the caller. The Anthropic
//! surface had the same defect in a different shape: its terminal
//! `message_delta` carried a hardcoded `output_tokens: 0`.

use bytes::Bytes;
use systemprompt_api::services::gateway::protocol::canonical::CanonicalContent;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;

fn snapshot() -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "test-model".into(),
        content: vec![CanonicalContent::Text("hi".into())],
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: CanonicalUsage {
            input_tokens: 11,
            output_tokens: 18,
            ..CanonicalUsage::default()
        },
        ..CanonicalResponse::default()
    }
}

fn text(bytes: &Bytes) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn chat_finish_chunk_states_no_usage() {
    let event = CanonicalEvent::MessageStop {
        id: "msg_1".into(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
    };
    let frame = OpenAiChatInbound
        .render_terminal_event(&event, &snapshot(), "test-model")
        .expect("the finish chunk is rendered from the snapshot");
    let rendered = text(&frame);
    assert!(rendered.contains("\"finish_reason\":\"stop\""), "{rendered}");
    assert!(
        !rendered.contains("usage"),
        "a zeroed usage object is worse than none; {rendered}"
    );
    assert!(
        !rendered.contains("[DONE]"),
        "the sentinel closes the stream after the usage chunk; {rendered}"
    );
}

#[test]
fn chat_tail_states_the_real_counts_then_the_sentinel() {
    let frame = OpenAiChatInbound
        .render_stream_tail(&snapshot(), true)
        .expect("chat always closes with a tail");
    let rendered = text(&frame);
    assert!(rendered.contains("\"choices\":[]"), "{rendered}");
    assert!(rendered.contains("\"prompt_tokens\":11"), "{rendered}");
    assert!(rendered.contains("\"completion_tokens\":18"), "{rendered}");
    let usage_at = rendered.find("usage").expect("usage chunk");
    let done_at = rendered.find("[DONE]").expect("sentinel");
    assert!(usage_at < done_at, "usage precedes the sentinel; {rendered}");
}

#[test]
fn chat_tail_without_include_usage_is_the_sentinel_alone() {
    let frame = OpenAiChatInbound
        .render_stream_tail(&snapshot(), false)
        .expect("chat always closes with a tail");
    let rendered = text(&frame);
    assert_eq!(rendered, "data: [DONE]\n\n");
}

#[test]
fn chat_reads_include_usage_off_the_caller_body() {
    let asked = Bytes::from_static(b"{\"stream\":true,\"stream_options\":{\"include_usage\":true}}");
    let silent = Bytes::from_static(b"{\"stream\":true}");
    assert!(OpenAiChatInbound.wants_stream_usage(&asked));
    assert!(!OpenAiChatInbound.wants_stream_usage(&silent));
}

// Why: the two surfaces that state usage inside their own terminal event have
// no tail to render, and rendering one would close their stream twice.
#[test]
fn the_other_surfaces_render_no_tail() {
    assert!(
        AnthropicMessagesInbound
            .render_stream_tail(&snapshot(), true)
            .is_none()
    );
    assert!(
        OpenAiResponsesInbound
            .render_stream_tail(&snapshot(), true)
            .is_none()
    );
}

#[test]
fn anthropic_terminal_delta_states_the_real_output_count() {
    let event = CanonicalEvent::MessageStop {
        id: "msg_1".into(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
    };
    let frame = AnthropicMessagesInbound
        .render_terminal_event(&event, &snapshot(), "test-model")
        .expect("the terminal pair is rendered from the snapshot");
    let rendered = text(&frame);
    assert!(rendered.contains("\"output_tokens\":18"), "{rendered}");
    assert!(rendered.contains("\"input_tokens\":11"), "{rendered}");
    assert!(rendered.contains("\"stop_reason\":\"end_turn\""), "{rendered}");
    assert!(rendered.contains("event: message_stop"), "{rendered}");
}
