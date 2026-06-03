use futures::StreamExt;
use serde_json::{Value, json};
use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason, ContentBlockKind};
use systemprompt_models::wire::{anthropic, gemini, openai_chat, openai_responses};

fn one_frame(sse: String) -> impl futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) })
}

fn chunks(parts: Vec<&str>) -> impl futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    let owned: Vec<bytes::Bytes> = parts
        .into_iter()
        .map(|p| bytes::Bytes::from(p.to_owned()))
        .collect();
    futures::stream::iter(owned.into_iter().map(Ok::<_, std::io::Error>))
}

mod anthropic_event_from_sse {
    use super::*;

    fn event(value: Value) -> Option<CanonicalEvent> {
        anthropic::event_from_sse(&value, "msg_1")
    }

    #[test]
    fn returns_none_when_type_missing() {
        assert!(event(json!({"foo": "bar"})).is_none());
    }

    #[test]
    fn returns_none_for_unknown_type() {
        assert!(event(json!({"type": "totally_unknown"})).is_none());
    }

    #[test]
    fn message_start_carries_id_model_usage() {
        let ev = event(json!({
            "type": "message_start",
            "message": {
                "id": "msg_abc",
                "model": "claude-3",
                "usage": {"input_tokens": 10, "output_tokens": 0}
            }
        }))
        .expect("event");
        match ev {
            CanonicalEvent::MessageStart { id, model, usage } => {
                assert_eq!(id, "msg_abc");
                assert_eq!(model, "claude-3");
                assert_eq!(usage.input_tokens, 10);
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn message_start_without_message_returns_none() {
        assert!(event(json!({"type": "message_start"})).is_none());
    }

    #[test]
    fn content_block_start_text() {
        let ev = event(json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text"}
        }))
        .expect("event");
        assert!(matches!(
            ev,
            CanonicalEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockKind::Text
            }
        ));
    }

    #[test]
    fn content_block_start_thinking_with_signature() {
        let ev = event(json!({
            "type": "content_block_start",
            "index": 2,
            "content_block": {"type": "thinking", "signature": "sig"}
        }))
        .expect("event");
        match ev {
            CanonicalEvent::ContentBlockStart {
                index,
                block: ContentBlockKind::Thinking { signature },
            } => {
                assert_eq!(index, 2);
                assert_eq!(signature.as_deref(), Some("sig"));
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn content_block_start_tool_use() {
        let ev = event(json!({
            "type": "content_block_start",
            "index": 1,
            "content_block": {"type": "tool_use", "id": "call_9", "name": "lookup"}
        }))
        .expect("event");
        match ev {
            CanonicalEvent::ContentBlockStart {
                index: 1,
                block: ContentBlockKind::ToolUse { id, name, .. },
            } => {
                assert_eq!(id, "call_9");
                assert_eq!(name, "lookup");
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn content_block_start_unknown_block_type_returns_none() {
        assert!(
            event(json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": {"type": "redacted_thinking"}
            }))
            .is_none()
        );
    }

    #[test]
    fn content_block_delta_text() {
        let ev = event(json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "hello"}
        }))
        .expect("event");
        assert!(matches!(
            ev,
            CanonicalEvent::TextDelta { index: 0, text } if text == "hello"
        ));
    }

    #[test]
    fn content_block_delta_thinking() {
        let ev = event(json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "thinking_delta", "thinking": "ponder"}
        }))
        .expect("event");
        assert!(matches!(
            ev,
            CanonicalEvent::ThinkingDelta { text, .. } if text == "ponder"
        ));
    }

    #[test]
    fn content_block_delta_signature() {
        let ev = event(json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "signature_delta", "signature": "abc=="}
        }))
        .expect("event");
        assert!(matches!(
            ev,
            CanonicalEvent::SignatureDelta { signature, .. } if signature == "abc=="
        ));
    }

    #[test]
    fn content_block_delta_input_json() {
        let ev = event(json!({
            "type": "content_block_delta",
            "index": 3,
            "delta": {"type": "input_json_delta", "partial_json": "{\"a\":"}
        }))
        .expect("event");
        assert!(matches!(
            ev,
            CanonicalEvent::ToolUseDelta { index: 3, partial_json } if partial_json == "{\"a\":"
        ));
    }

    #[test]
    fn content_block_delta_unknown_delta_type_returns_none() {
        assert!(
            event(json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "citations_delta"}
            }))
            .is_none()
        );
    }

    #[test]
    fn content_block_stop_carries_index() {
        let ev = event(json!({"type": "content_block_stop", "index": 5})).expect("event");
        assert!(matches!(ev, CanonicalEvent::ContentBlockStop { index: 5 }));
    }

    #[test]
    fn message_delta_with_stop_reason_emits_message_stop() {
        let ev = event(json!({
            "type": "message_delta",
            "delta": {"stop_reason": "tool_use"}
        }))
        .expect("event");
        match ev {
            CanonicalEvent::MessageStop { id, stop_reason } => {
                assert_eq!(id, "msg_1");
                assert_eq!(stop_reason, Some(CanonicalStopReason::ToolUse));
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn message_delta_with_usage_only_emits_usage_delta() {
        let ev = event(json!({
            "type": "message_delta",
            "usage": {"output_tokens": 42}
        }))
        .expect("event");
        match ev {
            CanonicalEvent::UsageDelta(usage) => assert_eq!(usage.output_tokens, 42),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn message_stop_uses_msg_id() {
        let ev = event(json!({"type": "message_stop"})).expect("event");
        assert!(matches!(
            ev,
            CanonicalEvent::MessageStop { id, stop_reason: None } if id == "msg_1"
        ));
    }

    #[test]
    fn error_extracts_message() {
        let ev = event(json!({
            "type": "error",
            "error": {"message": "overloaded"}
        }))
        .expect("event");
        assert!(matches!(ev, CanonicalEvent::Error(m) if m == "overloaded"));
    }

    #[test]
    fn error_without_message_uses_fallback() {
        let ev = event(json!({"type": "error", "error": {}})).expect("event");
        assert!(matches!(ev, CanonicalEvent::Error(m) if m == "upstream error"));
    }
}

mod anthropic_parse_response {
    use super::*;
    use systemprompt_models::wire::canonical::CanonicalContent;

    #[test]
    fn empty_object_uses_fallback_model() {
        let resp = anthropic::parse_response(&json!({}), "fallback-model");
        assert_eq!(resp.model, "fallback-model");
        assert!(resp.content.is_empty());
        assert!(resp.stop_reason.is_none());
    }

    #[test]
    fn parses_text_block_and_stop_reason() {
        let resp = anthropic::parse_response(
            &json!({
                "id": "msg_x",
                "model": "claude-3",
                "stop_reason": "end_turn",
                "content": [{"type": "text", "text": "hi there"}],
                "usage": {"input_tokens": 3, "output_tokens": 4}
            }),
            "fallback",
        );
        assert_eq!(resp.id, "msg_x");
        assert_eq!(resp.model, "claude-3");
        assert_eq!(resp.stop_reason, Some(CanonicalStopReason::EndTurn));
        assert_eq!(resp.usage.total_tokens, 7);
        assert!(matches!(
            resp.content.first(),
            Some(CanonicalContent::Text(t)) if t == "hi there"
        ));
        assert_eq!(resp.raw_finish_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn parses_tool_use_block() {
        let resp = anthropic::parse_response(
            &json!({
                "content": [{
                    "type": "tool_use", "id": "c1", "name": "go", "input": {"x": 1}
                }]
            }),
            "fb",
        );
        match resp.content.first() {
            Some(CanonicalContent::ToolUse {
                id, name, input, ..
            }) => {
                assert_eq!(id, "c1");
                assert_eq!(name, "go");
                assert_eq!(input["x"], json!(1));
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_thinking_block() {
        let resp = anthropic::parse_response(
            &json!({
                "content": [{"type": "thinking", "thinking": "hmm", "signature": "s"}]
            }),
            "fb",
        );
        assert!(matches!(
            resp.content.first(),
            Some(CanonicalContent::Thinking { text, signature })
                if text == "hmm" && signature.as_deref() == Some("s")
        ));
    }

    #[test]
    fn web_search_results_become_grounding_sources() {
        let resp = anthropic::parse_response(
            &json!({
                "content": [{
                    "type": "web_search_tool_result",
                    "content": [
                        {"type": "web_search_result", "url": "https://a.com", "title": "A"},
                        {"type": "web_search_result", "url": "", "title": "skip"}
                    ]
                }]
            }),
            "fb",
        );
        let grounding = resp.grounding.expect("grounding");
        assert_eq!(grounding.sources.len(), 1);
        assert_eq!(grounding.sources[0].uri, "https://a.com");
    }

    #[test]
    fn text_citations_become_grounding_sources() {
        let resp = anthropic::parse_response(
            &json!({
                "content": [{
                    "type": "text",
                    "text": "see source",
                    "citations": [{"url": "https://c.com", "title": "C", "cited_text": "x"}]
                }]
            }),
            "fb",
        );
        let grounding = resp.grounding.expect("grounding");
        assert_eq!(grounding.sources[0].uri, "https://c.com");
        assert_eq!(grounding.sources[0].snippet.as_deref(), Some("x"));
    }

    #[test]
    fn base64_image_block_defaults_media_type() {
        let resp = anthropic::parse_response(
            &json!({
                "content": [{
                    "type": "image",
                    "source": {"type": "base64", "data": "QQ=="}
                }]
            }),
            "fb",
        );
        match resp.content.first() {
            Some(CanonicalContent::Image(
                systemprompt_models::wire::canonical::ImageSource::Base64 {
                    media_type, data, ..
                },
            )) => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "QQ==");
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_block_type_is_dropped() {
        let resp = anthropic::parse_response(
            &json!({"content": [{"type": "future_thing", "x": 1}]}),
            "fb",
        );
        assert!(resp.content.is_empty());
    }
}

mod openai_chat_streaming {
    use super::*;

    async fn run(sse: String) -> Vec<CanonicalEvent> {
        openai_chat::sse_to_canonical_events(one_frame(sse), "fallback".to_owned())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("ok event"))
            .collect()
    }

    #[tokio::test]
    async fn text_delta_emits_start_delta_and_stop() {
        let sse = "data: {\"id\":\"c1\",\"model\":\"gpt\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n\
                   data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(matches!(events[0], CanonicalEvent::MessageStart { .. }));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::ContentBlockStart { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::TextDelta { text, .. } if text == "hi"))
        );
        assert!(events.iter().any(
            |e| matches!(e, CanonicalEvent::MessageStop { stop_reason, .. }
                    if *stop_reason == Some(CanonicalStopReason::EndTurn))
        ));
    }

    #[tokio::test]
    async fn done_sentinel_emits_message_stop() {
        let sse = "data: {\"id\":\"c1\",\"model\":\"gpt\",\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n\
                   data: [DONE]\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::MessageStop { .. })
        ));
    }

    #[tokio::test]
    async fn usage_chunk_emits_usage_delta() {
        let sse = "data: {\"id\":\"c1\",\"model\":\"gpt\",\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":7,\"total_tokens\":12},\"choices\":[]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::UsageDelta(u) if u.input_tokens == 5 && u.output_tokens == 7
        )));
    }

    #[tokio::test]
    async fn tool_call_delta_emits_block_start_and_args() {
        let sse = "data: {\"id\":\"c1\",\"model\":\"gpt\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"t1\",\"function\":{\"name\":\"go\",\"arguments\":\"{\\\"q\\\":1}\"}}]}}]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::ContentBlockStart { block: ContentBlockKind::ToolUse { name, .. }, .. }
                if name == "go"
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::ToolUseDelta { .. }))
        );
    }

    #[tokio::test]
    async fn malformed_json_frame_is_skipped() {
        let sse = "data: not-json\n\ndata: [DONE]\n\n".to_owned();
        let events = run(sse).await;
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::MessageStop { .. })
        ));
    }

    #[tokio::test]
    async fn empty_text_delta_does_not_open_block() {
        let sse = "data: {\"id\":\"c1\",\"model\":\"gpt\",\"choices\":[{\"delta\":{\"content\":\"\"}}]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::ContentBlockStart { .. }))
        );
    }
}

mod openai_responses_streaming {
    use super::*;

    async fn run(sse: String) -> Vec<CanonicalEvent> {
        openai_responses::sse_to_canonical_events(one_frame(sse), "fallback".to_owned())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("ok event"))
            .collect()
    }

    #[tokio::test]
    async fn created_emits_message_start() {
        let sse = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt\"}}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(matches!(
            events.first(),
            Some(CanonicalEvent::MessageStart { id, .. }) if id == "resp_1"
        ));
    }

    #[tokio::test]
    async fn message_item_text_delta_flow() {
        let sse = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"model\":\"gpt\"}}\n\n\
                   data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"message\"}}\n\n\
                   data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"abc\"}\n\n\
                   data: {\"type\":\"response.output_item.done\",\"output_index\":0}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::ContentBlockStart {
                block: ContentBlockKind::Text,
                ..
            }
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::TextDelta { text, .. } if text == "abc"))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::ContentBlockStop { .. }))
        );
    }

    #[tokio::test]
    async fn function_call_arguments_delta_flow() {
        let sse = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"model\":\"gpt\"}}\n\n\
                   data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"call_id\":\"c1\",\"name\":\"go\"}}\n\n\
                   data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"delta\":\"{}\"}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::ContentBlockStart { block: ContentBlockKind::ToolUse { name, .. }, .. }
                if name == "go"
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::ToolUseDelta { .. }))
        );
    }

    #[tokio::test]
    async fn reasoning_summary_delta_flow() {
        let sse = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"model\":\"gpt\"}}\n\n\
                   data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"reasoning\"}}\n\n\
                   data: {\"type\":\"response.reasoning_summary_text.delta\",\"output_index\":0,\"delta\":\"thinking\"}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(
            |e| matches!(e, CanonicalEvent::ThinkingDelta { text, .. } if text == "thinking")
        ));
    }

    #[tokio::test]
    async fn completed_emits_usage_and_message_stop() {
        let sse = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"model\":\"gpt\"}}\n\n\
                   data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4,\"total_tokens\":7}}}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::UsageDelta(u) if u.total_tokens == 7
        )));
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::MessageStop { .. })
        ));
    }

    #[tokio::test]
    async fn error_event_emits_error() {
        let sse = "data: {\"type\":\"error\",\"error\":{\"message\":\"boom\"}}\n\n".to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::Error(m) if m == "boom"
        )));
    }

    #[tokio::test]
    async fn failed_event_emits_error() {
        let sse =
            "data: {\"type\":\"response.failed\",\"error\":{\"message\":\"bad\"}}\n\n".to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(e, CanonicalEvent::Error(_))));
    }

    #[tokio::test]
    async fn unknown_event_type_is_ignored() {
        let sse = "data: {\"type\":\"response.in_progress\"}\n\n".to_owned();
        let events = run(sse).await;
        assert!(events.is_empty());
    }
}

mod gemini_streaming {
    use super::*;

    async fn run(sse: String) -> Vec<CanonicalEvent> {
        gemini::sse_to_canonical_events(one_frame(sse), "fallback".to_owned())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("ok event"))
            .collect()
    }

    #[tokio::test]
    async fn text_part_emits_start_and_delta() {
        let sse = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hello\"}]}}]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(matches!(events[0], CanonicalEvent::MessageStart { .. }));
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::ContentBlockStart {
                block: ContentBlockKind::Text,
                ..
            }
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::TextDelta { text, .. } if text == "hello"))
        );
    }

    #[tokio::test]
    async fn finish_reason_emits_message_stop() {
        let sse = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"x\"}]},\"finishReason\":\"STOP\"}]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(matches!(
            events.last(),
            Some(CanonicalEvent::MessageStop { .. })
        ));
    }

    #[tokio::test]
    async fn usage_metadata_emits_usage_delta() {
        let sse = "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":4,\"candidatesTokenCount\":6}}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(events.iter().any(|e| matches!(
            e,
            CanonicalEvent::UsageDelta(u) if u.input_tokens == 4 && u.output_tokens == 6
        )));
    }

    #[tokio::test]
    async fn malformed_chunk_skipped() {
        let sse = "data: not-json\n\n".to_owned();
        let events = run(sse).await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn empty_text_part_does_not_open_block() {
        let sse = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"\"}]}}]}\n\n"
            .to_owned();
        let events = run(sse).await;
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, CanonicalEvent::ContentBlockStart { .. }))
        );
    }
}

mod crlf_framing {
    use super::*;

    const GEMINI_CRLF: &str = "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hi\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":8,\"candidatesTokenCount\":7},\"modelVersion\":\"gemini-2.5-flash\",\"responseId\":\"r1\"}\r\n\r\n";
    const OPENAI_CHAT_CRLF: &str = "data: {\"id\":\"c1\",\"model\":\"gpt\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\r\n\r\n";
    const OPENAI_RESPONSES_CRLF: &str = "data: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"model\":\"gpt\"}}\r\n\r\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"message\"}}\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"hi\"}\r\n\r\n";

    fn has_text(events: &[CanonicalEvent], expected: &str) -> bool {
        events
            .iter()
            .any(|e| matches!(e, CanonicalEvent::TextDelta { text, .. } if text == expected))
    }

    async fn collect_gemini<S>(stream: S) -> Vec<CanonicalEvent>
    where
        S: futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + 'static,
    {
        gemini::sse_to_canonical_events(stream, "fallback".to_owned())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("ok event"))
            .collect()
    }

    async fn collect_openai_chat<S>(stream: S) -> Vec<CanonicalEvent>
    where
        S: futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + 'static,
    {
        openai_chat::sse_to_canonical_events(stream, "fallback".to_owned())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("ok event"))
            .collect()
    }

    async fn collect_openai_responses<S>(stream: S) -> Vec<CanonicalEvent>
    where
        S: futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send + 'static,
    {
        openai_responses::sse_to_canonical_events(stream, "fallback".to_owned())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| r.expect("ok event"))
            .collect()
    }

    #[tokio::test]
    async fn gemini_crlf_frame_yields_events() {
        let events = collect_gemini(one_frame(GEMINI_CRLF.to_owned())).await;
        assert!(matches!(events.first(), Some(CanonicalEvent::MessageStart { .. })));
        assert!(has_text(&events, "hi"));
        assert!(matches!(events.last(), Some(CanonicalEvent::MessageStop { .. })));
    }

    #[tokio::test]
    async fn gemini_crlf_split_across_chunks_yields_events() {
        let mid = GEMINI_CRLF.len() - 2;
        let (a, b) = GEMINI_CRLF.split_at(mid);
        let events = collect_gemini(chunks(vec![a, b])).await;
        assert!(has_text(&events, "hi"));
        assert!(matches!(events.last(), Some(CanonicalEvent::MessageStop { .. })));
    }

    #[tokio::test]
    async fn openai_chat_crlf_frame_yields_events() {
        let events = collect_openai_chat(one_frame(OPENAI_CHAT_CRLF.to_owned())).await;
        assert!(has_text(&events, "hi"));
        assert!(matches!(events.last(), Some(CanonicalEvent::MessageStop { .. })));
    }

    #[tokio::test]
    async fn openai_responses_crlf_frame_yields_events() {
        let events = collect_openai_responses(one_frame(OPENAI_RESPONSES_CRLF.to_owned())).await;
        assert!(matches!(events.first(), Some(CanonicalEvent::MessageStart { .. })));
        assert!(has_text(&events, "hi"));
    }
}
