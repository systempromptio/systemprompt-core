//! What each inbound surface says when a stream aborts.
//!
//! The stream tap renders [`STREAM_ABORT_MESSAGE`] through the inbound
//! adapter's own error frame when an upstream ends with no terminal event, so
//! these three frames are the whole of what a client sees on an aborted turn.
//! Each must be the failure signal that surface's clients actually read: an
//! `error` event on Anthropic, an error chunk followed by the `[DONE]`
//! sentinel on Chat Completions, and a `response.failed` carrying
//! `status: "failed"` on Responses.

use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::protocol::canonical_response::CanonicalEvent;
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_api::services::gateway::stream_tap::STREAM_ABORT_MESSAGE;

fn abort_frame(inbound: &dyn InboundAdapter) -> String {
    let event = CanonicalEvent::Error(STREAM_ABORT_MESSAGE.to_owned());
    let bytes = inbound
        .render_event(&event, "test-model")
        .expect("every inbound surface must state a stream abort");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn anthropic_abort_is_an_error_event() {
    let frame = abort_frame(&AnthropicMessagesInbound);
    assert!(frame.starts_with("event: error\n"), "{frame}");
    assert!(frame.contains("\"type\":\"api_error\""), "{frame}");
    assert!(frame.contains(STREAM_ABORT_MESSAGE), "{frame}");
}

#[test]
fn openai_chat_abort_closes_with_the_done_sentinel() {
    let frame = abort_frame(&OpenAiChatInbound);
    assert!(frame.contains("\"type\":\"upstream_error\""), "{frame}");
    assert!(frame.contains(STREAM_ABORT_MESSAGE), "{frame}");
    assert!(
        frame.trim_end().ends_with("data: [DONE]"),
        "an OpenAI-SDK client reads until the sentinel; {frame}"
    );
}

#[test]
fn openai_responses_abort_sets_response_status_failed() {
    let frame = abort_frame(&OpenAiResponsesInbound);
    assert!(frame.starts_with("event: response.failed\n"), "{frame}");
    assert!(frame.contains("\"status\":\"failed\""), "{frame}");
    assert!(frame.contains(STREAM_ABORT_MESSAGE), "{frame}");
}

// Why: the message is what a human reading a client log has to act on, and it
// is the same string the audit row carries for a truncated stream.
#[test]
fn the_abort_message_names_the_missing_terminal() {
    assert_eq!(
        STREAM_ABORT_MESSAGE,
        "upstream stream ended without a terminal event"
    );
}
