//! The default method bodies on `InboundAdapter`.
//!
//! Each adapter overrides a different subset, so the defaults only run for the
//! adapters that leave them alone — and no test exercised those combinations.
//! The defaults decide whether a request is forwarded byte-for-byte, whether a
//! terminal SSE frame is specially rendered, and what content type a stream is
//! served as, so a wrong default is a silent wire change.

use bytes::Bytes;
use http::StatusCode;
use systemprompt_api::services::gateway::protocol::canonical::CanonicalRequest;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};
use systemprompt_api::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_api::services::gateway::protocol::inbound::{InboundAdapter, InboundParseError};

fn snapshot() -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_default".into(),
        model: "m".into(),
        content: Vec::new(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: CanonicalUsage::default(),
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
        ..Default::default()
    }
}

#[test]
fn an_adapter_that_declares_no_passthrough_wire_reports_none() {
    // Anthropic opts into byte-for-byte forwarding; the Responses wire must
    // rebuild from canonical, so it must not claim a passthrough.
    assert!(
        OpenAiResponsesInbound.passthrough_wire().is_none(),
        "a rebuilt wire must never be forwarded verbatim"
    );
    assert!(
        AnthropicMessagesInbound.passthrough_wire().is_some(),
        "the Anthropic wire opts into passthrough so beta-gated fields survive"
    );
}

// Why: all three shipped adapters now render their own terminal, because all
// three state the turn's usage on it. The default is what any adapter added
// later inherits, so it is exercised through a minimal one rather than left
// unproven.
#[derive(Debug)]
struct BareInbound;

impl InboundAdapter for BareInbound {
    fn wire_name(&self) -> &'static str {
        "test.bare"
    }

    fn parse_request(&self, _raw: &Bytes) -> Result<CanonicalRequest, InboundParseError> {
        Err(InboundParseError::MissingField("model"))
    }

    fn render_response(&self, _response: &CanonicalResponse) -> Bytes {
        Bytes::new()
    }

    fn render_event(&self, _event: &CanonicalEvent, _model: &str) -> Option<Bytes> {
        None
    }

    fn render_error(&self, _status: StatusCode, _message: &str) -> Bytes {
        Bytes::new()
    }
}

#[test]
fn an_adapter_with_no_terminal_frame_falls_back_to_the_per_event_render() {
    let event = CanonicalEvent::MessageStop {
        id: "msg_default".to_owned(),
        stop_reason: Some(CanonicalStopReason::EndTurn),
    };

    assert!(
        BareInbound
            .render_terminal_event(&event, &snapshot(), "m")
            .is_none(),
        "returning None is what routes the caller back to render_event"
    );
    assert!(
        BareInbound.render_stream_tail(&snapshot(), true).is_none(),
        "a wire that states no closing frames must not invent one"
    );
    assert!(
        !BareInbound.wants_stream_usage(&Bytes::from_static(b"{}")),
        "asking for streamed usage is a Chat Completions concept"
    );
}

#[test]
fn every_adapter_streams_as_server_sent_events_by_default() {
    assert_eq!(
        AnthropicMessagesInbound.streaming_content_type(),
        "text/event-stream"
    );
    assert_eq!(
        OpenAiResponsesInbound.streaming_content_type(),
        "text/event-stream"
    );
}

#[test]
fn each_adapter_names_its_own_wire() {
    assert_ne!(
        AnthropicMessagesInbound.wire_name(),
        OpenAiResponsesInbound.wire_name(),
        "the wire name is recorded on the audit row; two wires must not share one"
    );
}
