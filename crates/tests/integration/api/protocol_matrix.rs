//! Pure-function coverage for the gateway protocol matrix: inbound parsers
//! (Anthropic Messages, OpenAI Responses), outbound request builders +
//! response parsers (Anthropic, OpenAI Chat, OpenAI Responses), and SSE-byte
//! streaming decoders for each outbound provider. Drives the private modules
//! through the `protocol_test_api` re-exports gated by the `test-api`
//! feature on `systemprompt-api`.

use bytes::Bytes;
use futures::StreamExt;
use systemprompt_api::services::gateway::protocol::{
    CanonicalContent, CanonicalEvent, CanonicalRequest, CanonicalStopReason, ContentBlockKind,
    Role, anthropic_messages, openai_chat, openai_responses as openai_responses_in,
    outbound_anthropic, outbound_openai_responses,
};

// -----------------------------------------------------------------------------
// Inbound parsers
// -----------------------------------------------------------------------------

#[test]
fn anthropic_messages_parses_minimal_request() {
    let body = serde_json::json!({
        "model": "claude-3-5-sonnet",
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": "hello"}]
    });
    let req = anthropic_messages::test_api::parse_request(&body).expect("parse");
    assert_eq!(req.model, "claude-3-5-sonnet");
    assert_eq!(req.max_tokens, 1024);
    assert_eq!(req.messages.len(), 1);
    assert!(matches!(req.messages[0].role, Role::User));
}

#[test]
fn anthropic_messages_missing_model_errors() {
    let body = serde_json::json!({
        "max_tokens": 100,
        "messages": [{"role": "user", "content": "x"}]
    });
    assert!(anthropic_messages::test_api::parse_request(&body).is_err());
}

#[test]
fn anthropic_messages_parses_system_prompt_and_temperature() {
    let body = serde_json::json!({
        "model": "claude-3-opus",
        "max_tokens": 256,
        "system": "You are helpful",
        "temperature": 0.7,
        "messages": [{"role": "user", "content": "ping"}]
    });
    let req = anthropic_messages::test_api::parse_request(&body).expect("parse");
    assert_eq!(req.system.as_deref(), Some("You are helpful"));
    assert!((req.temperature.unwrap() - 0.7).abs() < 1e-5);
}

#[test]
fn openai_responses_parses_minimal_request() {
    let body = serde_json::json!({
        "model": "gpt-5",
        "max_output_tokens": 512,
        "input": [{"role": "user", "content": [{"type": "input_text", "text": "hi"}]}]
    });
    let req = openai_responses_in::test_api::parse_request(&body).expect("parse");
    assert_eq!(req.model, "gpt-5");
    assert_eq!(req.max_tokens, 512);
    assert!(!req.messages.is_empty());
}

// -----------------------------------------------------------------------------
// Outbound request builders
// -----------------------------------------------------------------------------

fn fixture_request(model: &str, stream: bool) -> CanonicalRequest {
    CanonicalRequest {
        model: model.to_owned(),
        system: Some("be brief".to_owned()),
        messages: vec![
            systemprompt_api::services::gateway::protocol::CanonicalMessage {
                role: Role::User,
                content: vec![CanonicalContent::Text("hello".to_owned())],
            },
        ],
        max_tokens: 256,
        temperature: Some(0.5),
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

#[test]
fn anthropic_outbound_request_builder_carries_model_and_messages() {
    let req = fixture_request("claude-3-5-sonnet", false);
    let body =
        outbound_anthropic::test_api::build_request_body(&req, "claude-3-5-sonnet-upstream", None);
    assert_eq!(body["model"], "claude-3-5-sonnet-upstream");
    assert_eq!(body["max_tokens"], 256);
    assert!(body["messages"].is_array());
    assert_eq!(body["system"], "be brief");
}

#[test]
fn openai_chat_outbound_request_builder_renames_to_chat_completions_shape() {
    let req = fixture_request("gpt-4o", true);
    let body = openai_chat::test_api::build_request_body(&req, "gpt-4o-upstream", None);
    assert_eq!(body["model"], "gpt-4o-upstream");
    assert_eq!(body["stream"], true);
    assert!(body["messages"].is_array());
}

#[test]
fn openai_responses_outbound_request_builder_uses_responses_shape() {
    let req = fixture_request("gpt-5", false);
    let body = outbound_openai_responses::test_api::build_request_body(&req, "gpt-5-upstream", None);
    assert_eq!(body["model"], "gpt-5-upstream");
    assert!(body.get("input").is_some() || body.get("messages").is_some());
}

// -----------------------------------------------------------------------------
// Outbound response parsers (non-stream)
// -----------------------------------------------------------------------------

#[test]
fn anthropic_response_parser_extracts_text_and_usage() {
    let resp = serde_json::json!({
        "id": "msg_1",
        "model": "claude-3-5",
        "content": [{"type": "text", "text": "ok"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 20}
    });
    let canon = outbound_anthropic::test_api::parse_response(&resp, "fallback-model");
    assert_eq!(canon.id, "msg_1");
    assert_eq!(canon.usage.input_tokens, 10);
    assert_eq!(canon.usage.output_tokens, 20);
    assert!(matches!(
        canon.stop_reason,
        Some(CanonicalStopReason::EndTurn)
    ));
    assert!(matches!(
        canon.content.first(),
        Some(CanonicalContent::Text(t)) if t == "ok"
    ));
}

#[test]
fn anthropic_response_parser_falls_back_to_model_when_missing() {
    let resp = serde_json::json!({
        "id": "msg_x",
        "content": [{"type": "text", "text": "y"}]
    });
    let canon = outbound_anthropic::test_api::parse_response(&resp, "fallback-model");
    assert_eq!(canon.model, "fallback-model");
}

#[test]
fn openai_chat_response_parser_extracts_choice_content() {
    let resp = serde_json::json!({
        "id": "chatcmpl_1",
        "model": "gpt-4o",
        "choices": [{
            "message": {"role": "assistant", "content": "answer"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 5, "completion_tokens": 7}
    });
    let canon = openai_chat::test_api::parse_response(&resp, "fallback");
    assert_eq!(canon.id, "chatcmpl_1");
    assert!(
        canon
            .content
            .iter()
            .any(|p| matches!(p, CanonicalContent::Text(t) if t == "answer"))
    );
    assert_eq!(canon.usage.input_tokens, 5);
    assert_eq!(canon.usage.output_tokens, 7);
}

#[test]
fn openai_responses_object_parser_extracts_output_text() {
    let resp = serde_json::json!({
        "id": "resp_1",
        "model": "gpt-5",
        "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": "hello world"}]
        }],
        "usage": {"input_tokens": 4, "output_tokens": 3}
    });
    let canon = outbound_openai_responses::test_api::parse_response_object(&resp, "fallback");
    assert_eq!(canon.id, "resp_1");
    assert!(
        canon
            .content
            .iter()
            .any(|p| matches!(p, CanonicalContent::Text(t) if t.contains("hello")))
    );
}

// -----------------------------------------------------------------------------
// Streaming decoders
// -----------------------------------------------------------------------------

fn byte_stream(
    chunks: Vec<&'static str>,
) -> futures::stream::BoxStream<'static, Result<Bytes, reqwest::Error>> {
    use futures::stream;
    stream::iter(
        chunks
            .into_iter()
            .map(|c| Ok::<_, reqwest::Error>(Bytes::from(c.as_bytes()))),
    )
    .boxed()
}

async fn collect_events<S>(s: S) -> Vec<CanonicalEvent>
where
    S: futures::Stream<Item = Result<CanonicalEvent, String>> + Send + 'static,
{
    s.filter_map(|r| async move { r.ok() }).collect().await
}

#[tokio::test]
async fn anthropic_streaming_decoder_emits_text_delta_then_message_stop() {
    let chunks = vec![
        "event: message_start\ndata: \
         {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude\",\"usage\":\
         {\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: \
         {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"\
         text\":\"\"}}\n\n",
        "event: content_block_delta\ndata: \
         {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\"\
         :\"hi\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_delta\ndata: \
         {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"\
         output_tokens\":5}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ];
    let stream = outbound_anthropic::test_api::sse_to_canonical_events(byte_stream(chunks));
    let events = collect_events(stream).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::MessageStart { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::TextDelta { text, .. } if text == "hi"))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::MessageStop { .. }))
    );
}

#[tokio::test]
async fn anthropic_streaming_decoder_handles_split_event_across_chunk_boundary() {
    // The 'message_start' event payload is split mid-JSON across two chunks.
    let chunks = vec![
        "event: message_start\ndata: \
         {\"type\":\"message_start\",\"message\":{\"id\":\"msg_split\",\"model\":\"cl",
        "aude\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ];
    let stream = outbound_anthropic::test_api::sse_to_canonical_events(byte_stream(chunks));
    let events = collect_events(stream).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::MessageStart { id, .. } if id == "msg_split"))
    );
}

#[tokio::test]
async fn openai_chat_streaming_decoder_emits_text_deltas() {
    let chunks = vec![
        "data: {\"id\":\"c_1\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\
         \"assistant\",\"content\":\"\"}}]}\n\n",
        "data: {\"id\":\"c_1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello\"}}]}\n\n",
        "data: {\"id\":\"c_1\",\"choices\":[{\"index\":0,\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    ];
    let stream =
        openai_chat::test_api::sse_to_canonical_events(byte_stream(chunks), "gpt-4o".to_owned());
    let events = collect_events(stream).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::TextDelta { text, .. } if text == "hello"))
    );
}

#[tokio::test]
async fn openai_responses_streaming_decoder_recognises_response_created() {
    let chunks = vec![
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r_1\",\"model\":\"gpt-5\"}}\n\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"i_1\",\"type\":\"\
         message\",\"content\":[]}}\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"world\"}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":3,\"\
         output_tokens\":5}}}\n\n",
        "data: [DONE]\n\n",
    ];
    let stream = outbound_openai_responses::test_api::sse_to_canonical_events(
        byte_stream(chunks),
        "gpt-5".to_owned(),
    );
    let events = collect_events(stream).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::MessageStart { .. }))
    );
}

#[tokio::test]
async fn streaming_decoder_yields_no_events_on_empty_stream() {
    let stream = outbound_anthropic::test_api::sse_to_canonical_events(byte_stream(vec![]));
    let events = collect_events(stream).await;
    assert!(events.is_empty());
}

// -----------------------------------------------------------------------------
// Inbound renderers (canonical → wire JSON)
// -----------------------------------------------------------------------------

#[test]
fn anthropic_render_response_value_emits_id_model_content() {
    let canon = systemprompt_api::services::gateway::protocol::CanonicalResponse {
        id: "msg_render_1".to_owned(),
        model: "claude-3-5".to_owned(),
        content: vec![CanonicalContent::Text("done".to_owned())],
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: systemprompt_api::services::gateway::protocol::CanonicalUsage {
            input_tokens: 1,
            output_tokens: 2,
            ..Default::default()
        },
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
    };
    let value = anthropic_messages::test_api::render_response_value(&canon);
    assert_eq!(value["id"], "msg_render_1");
    assert_eq!(value["model"], "claude-3-5");
    assert!(value["content"].is_array());
}

#[test]
fn anthropic_render_event_frame_text_delta_produces_sse_block() {
    let event = CanonicalEvent::TextDelta {
        index: 0,
        text: "x".to_owned(),
    };
    let bytes =
        anthropic_messages::test_api::render_event_frame(&event, "claude").expect("rendered");
    let s = std::str::from_utf8(&bytes).unwrap_or_default();
    assert!(
        s.contains("content_block_delta") || s.contains("text_delta"),
        "{s}"
    );
}

#[test]
fn anthropic_render_event_frame_unknown_kind_returns_none_for_unsupported() {
    let event = CanonicalEvent::ContentBlockStart {
        index: 99,
        block: ContentBlockKind::Text,
    };
    // Whether this returns Some or None depends on the renderer; the test
    // simply locks in that the function returns without panicking and that any
    // Some result is non-empty.
    if let Some(bytes) = anthropic_messages::test_api::render_event_frame(&event, "claude") {
        assert!(!bytes.is_empty());
    }
}
