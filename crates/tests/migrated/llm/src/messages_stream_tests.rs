//! The messages-protocol stream decoder: text, reasoning, tool input, usage,
//! stop reasons and error frames.

use serde_json::json;
use systemprompt_engine::llm::{Content, Event, FinishReason, LlmError, Message, Protocol};

use crate::support::{
    all_supports, body, decode, decode_ok, errors, finish, message_end, message_start,
    messages_answer, messages_sse, oks, request, text_block, texts, tool_block, tool_ends,
};

#[tokio::test]
async fn text_reasoning_and_usage_decode_from_one_stream() {
    let mut content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "weighing"}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "sig-1"}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    content.extend(text_block(1, "answer"));
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert_eq!(
        events[0],
        Event::ReasoningDelta {
            text: "weighing".into(),
            signature: None
        }
    );
    assert!(events.contains(&Event::ReasoningDelta {
        text: String::new(),
        signature: Some("sig-1".into())
    }));
    assert_eq!(texts(&events), "answer");
    let usage = events
        .iter()
        .find_map(|e| match e {
            Event::Usage(u) => Some(*u),
            _ => None,
        })
        .unwrap();
    assert_eq!((usage.input, usage.output), (10, 5));
    assert_eq!(finish(&events), Some(&FinishReason::Stop));
}

#[tokio::test]
async fn a_usage_only_delta_after_the_stop_reason_keeps_it() {
    let mut events = vec![message_start(7)];
    events.extend(text_block(0, "hi"));
    events.push(json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 3}}));
    events.push(json!({"type": "message_delta", "delta": {}, "usage": {"output_tokens": 9}}));
    events.push(json!({"type": "message_stop"}));
    let decoded = decode_ok(Protocol::Messages, &messages_sse(&events)).await;
    assert_eq!(finish(&decoded), Some(&FinishReason::ToolCalls));
    let usage = decoded
        .iter()
        .find_map(|e| match e {
            Event::Usage(u) => Some(*u),
            _ => None,
        })
        .unwrap();
    assert_eq!((usage.input, usage.output), (7, 9));
}

#[tokio::test]
#[ignore = "parity ledger: a stream that states its stop reason but never sends message_stop is finished, not failed"]
async fn a_stream_without_message_stop_is_not_complete() {
    let mut events = vec![message_start(1)];
    events.extend(text_block(0, "hi"));
    events.push(json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 1}}));
    let decoded = decode(Protocol::Messages, &messages_sse(&events)).await;
    assert!(matches!(errors(&decoded).as_slice(), [LlmError::Truncated]));
}

#[tokio::test]
async fn unknown_named_events_are_ignored() {
    let mut content = vec![
        json!({"type": "ping"}),
        json!({"type": "future_event", "payload": {"x": 1}}),
    ];
    content.extend(text_block(0, "still here"));
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert_eq!(texts(&events), "still here");
    assert_eq!(finish(&events), Some(&FinishReason::Stop));
}

#[tokio::test]
async fn unknown_block_and_delta_variants_are_ignored() {
    let mut content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "hologram", "data": 1}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "hologram_delta", "data": 2}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    content.extend(text_block(1, "ok"));
    content.push(
        json!({"type": "content_block_delta", "index": 1, "delta": {"type": "sparkle_delta"}}),
    );
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert_eq!(texts(&events), "ok");
}

#[tokio::test]
#[ignore = "parity ledger: a known content block whose fields have the wrong type is skipped, not rejected"]
async fn a_malformed_known_block_is_rejected() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": 42}}),
    ];
    let decoded = decode(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: a known delta whose fields have the wrong type is skipped, not rejected"]
async fn a_malformed_known_delta_is_rejected() {
    let mut content = text_block(0, "a").to_vec();
    content.insert(1, json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": ["not", "text"]}}));
    let decoded = decode(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: a malformed payload on a known non-content event (message_start) is ignored, not rejected"]
async fn a_malformed_payload_on_another_event_is_rejected() {
    let events = vec![json!({"type": "message_start", "message": "not an object"})];
    let mut all = events;
    all.extend(text_block(0, "a"));
    all.extend(message_end("end_turn", 1));
    let decoded = decode(Protocol::Messages, &messages_sse(&all)).await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
async fn an_event_whose_data_is_not_json_fails_the_stream_as_a_decode_error() {
    let body = format!(
        "{}event: content_block_delta\ndata: {{not json\n\n",
        messages_sse(&[message_start(1)])
    );
    let decoded = decode(Protocol::Messages, &body).await;
    assert!(matches!(errors(&decoded).as_slice(), [LlmError::Decode(_)]));
}

#[tokio::test]
async fn a_null_input_token_count_is_tolerated() {
    let events = vec![
        json!({"type": "message_start", "message": {"usage": {"input_tokens": null, "output_tokens": 1, "server_tool_use": {"web": 0}}}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 4}}),
        json!({"type": "message_stop"}),
    ];
    let decoded = decode_ok(Protocol::Messages, &messages_sse(&events)).await;
    let usage = decoded
        .iter()
        .find_map(|e| match e {
            Event::Usage(u) => Some(*u),
            _ => None,
        })
        .unwrap();
    assert_eq!(usage.output, 4);
}

#[tokio::test]
async fn a_reasoning_block_with_only_a_signature_closes_with_that_signature() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "only-sig"}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::ReasoningDelta { .. }))
            .collect::<Vec<_>>(),
        vec![&Event::ReasoningDelta {
            text: String::new(),
            signature: Some("only-sig".into())
        }]
    );
}

#[tokio::test]
async fn a_signature_given_at_block_start_is_kept() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "t", "signature": "start-sig"}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert!(events.contains(&Event::ReasoningDelta {
        text: String::new(),
        signature: Some("start-sig".into())
    }));
}

#[tokio::test]
async fn complete_tool_input_given_at_block_start_is_kept() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t1", "name": "read", "input": {"path": "a.txt"}}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await;
    assert_eq!(
        tool_ends(&events),
        vec![("t1".into(), "read".into(), json!({"path": "a.txt"}), None)]
    );
}

#[tokio::test]
#[ignore = "parity ledger: an empty text block produces no event, so nothing marks that the block existed"]
async fn an_empty_text_block_is_kept() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert!(events.contains(&Event::TextDelta(String::new())));
}

#[tokio::test]
async fn redacted_reasoning_and_a_tool_call_decode_and_go_back_in_the_next_request() {
    let mut content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "redacted_thinking", "data": "opaque"}}),
        json!({"type": "content_block_stop", "index": 0}),
    ];
    content.extend(tool_block(1, "t1", "read", &["{\"path\":", "\"a\"}"]));
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await;
    assert!(events.contains(&Event::RedactedReasoning {
        data: "opaque".into()
    }));
    let (id, name, input, _) = tool_ends(&events).remove(0);
    let mut next = request(Protocol::Messages, all_supports());
    next.messages = vec![
        Message::user_text("go"),
        Message::assistant(vec![
            Content::RedactedReasoning {
                data: "opaque".into(),
            },
            Content::ToolCall {
                id: id.clone(),
                name,
                input,
            },
        ]),
        Message::user(vec![Content::ToolResult {
            id,
            output: "done".into(),
            is_error: false,
            attachments: vec![],
        }]),
    ];
    let shaped = body(&next);
    let assistant = &shaped["messages"][1]["content"];
    assert_eq!(
        assistant[0],
        json!({"type": "redacted_thinking", "data": "opaque"})
    );
    assert_eq!(assistant[1]["type"], "tool_use");
    assert_eq!(assistant[1]["input"], json!({"path": "a"}));
}

#[tokio::test]
async fn running_out_of_context_window_finishes_as_length() {
    let events = decode_ok(
        Protocol::Messages,
        &messages_answer(&text_block(0, "x"), "model_context_window_exceeded"),
    )
    .await;
    assert_eq!(finish(&events), Some(&FinishReason::Length));
}

#[tokio::test]
async fn a_paused_turn_finishes_as_stop() {
    let events = decode_ok(
        Protocol::Messages,
        &messages_answer(&text_block(0, "x"), "pause_turn"),
    )
    .await;
    assert_eq!(finish(&events), Some(&FinishReason::Stop));
}

#[tokio::test]
async fn a_refusal_finishes_as_content_filter() {
    let events = decode_ok(
        Protocol::Messages,
        &messages_answer(&text_block(0, "no"), "refusal"),
    )
    .await;
    assert_eq!(finish(&events), Some(&FinishReason::ContentFilter));
}

#[tokio::test]
async fn input_deltas_for_a_block_never_started_are_ignored() {
    let mut content = vec![
        json!({"type": "content_block_delta", "index": 4, "delta": {"type": "input_json_delta", "partial_json": "{\"x\":1}"}}),
    ];
    content.extend(text_block(0, "fine"));
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert!(tool_ends(&events).is_empty());
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::ToolCallDelta { .. }))
    );
}

#[tokio::test]
#[ignore = "parity ledger: a tool block left open when message_stop arrives is dropped instead of being settled"]
async fn an_open_tool_block_is_settled_at_message_stop() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t1", "name": "read", "input": {}}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":\"a\"}"}}),
    ];
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await;
    assert_eq!(tool_ends(&events).len(), 1);
}

#[tokio::test]
async fn several_tool_calls_in_one_answer_are_each_assembled() {
    let mut content = tool_block(0, "t1", "read", &["{\"path\":\"a\"}"]);
    content.extend(tool_block(1, "t2", "grep", &["{\"pattern\":", "\"b\"}"]));
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await;
    assert_eq!(
        tool_ends(&events),
        vec![
            ("t1".into(), "read".into(), json!({"path": "a"}), None),
            ("t2".into(), "grep".into(), json!({"pattern": "b"}), None),
        ]
    );
}

#[tokio::test]
async fn a_frame_without_a_type_or_name_is_ignored() {
    let body = format!(
        "data: {{\"unrelated\": true}}\n\n{}",
        messages_answer(&text_block(0, "ok"), "end_turn")
    );
    let events = decode_ok(Protocol::Messages, &body).await;
    assert_eq!(texts(&events), "ok");
}

#[tokio::test]
#[ignore = "parity ledger: an error envelope sent without a type or event name is ignored instead of failing the stream"]
async fn an_untyped_error_envelope_fails_the_stream() {
    let body = format!(
        "{}data: {{\"error\": {{\"type\": \"overloaded_error\", \"message\": \"busy\"}}}}\n\n",
        messages_sse(&[message_start(1)])
    );
    let decoded = decode(Protocol::Messages, &body).await;
    assert!(matches!(
        errors(&decoded).as_slice(),
        [LlmError::Provider { .. }]
    ));
}

#[tokio::test]
async fn a_prompt_too_long_error_frame_is_a_context_overflow() {
    let body = messages_sse(&[
        message_start(1),
        json!({"type": "error", "error": {"type": "invalid_request_error", "message": "prompt is too long: 210000 tokens > 200000 maximum"}}),
    ]);
    let decoded = decode(Protocol::Messages, &body).await;
    assert!(matches!(
        errors(&decoded).as_slice(),
        [LlmError::ContextOverflow { .. }]
    ));
}

#[tokio::test]
async fn an_error_frame_without_a_message_reports_its_type() {
    let body = messages_sse(&[json!({"type": "error", "error": {"type": "overloaded_error"}})]);
    let decoded = decode(Protocol::Messages, &body).await;
    match errors(&decoded).as_slice() {
        [LlmError::Provider { kind, message }] => {
            assert_eq!(kind.as_deref(), Some("overloaded_error"));
            assert_eq!(message, "overloaded_error");
        },
        other => panic!("expected a provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn an_error_frame_without_a_payload_has_a_stable_message() {
    let body = messages_sse(&[json!({"type": "error"})]);
    let decoded = decode(Protocol::Messages, &body).await;
    match errors(&decoded).as_slice() {
        [
            LlmError::Provider {
                kind: None,
                message,
            },
        ] => assert_eq!(message, "unknown stream error"),
        other => panic!("expected a provider error, got {other:?}"),
    }
}

#[tokio::test]
async fn text_and_reasoning_arrive_in_the_order_they_were_sent() {
    let content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "one"}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": "two"}}),
        json!({"type": "content_block_stop", "index": 1}),
        json!({"type": "content_block_start", "index": 2, "content_block": {"type": "thinking", "thinking": "three"}}),
        json!({"type": "content_block_stop", "index": 2}),
    ];
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    let order: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::TextDelta(t) => Some(t.as_str()),
            Event::ReasoningDelta { text, .. } if !text.is_empty() => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(order, ["one", "two", "three"]);
}

#[tokio::test]
async fn a_long_text_stream_is_delivered_whole() {
    let mut content = vec![
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
    ];
    let mut expected = String::new();
    for n in 0..500 {
        let piece = format!("w{n} ");
        expected.push_str(&piece);
        content.push(json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": piece}}));
    }
    content.push(json!({"type": "content_block_stop", "index": 0}));
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "end_turn")).await;
    assert_eq!(texts(&events), expected);
}

#[tokio::test]
async fn usage_from_the_start_is_kept_when_the_end_omits_it() {
    let mut events = vec![
        json!({"type": "message_start", "message": {"usage": {"input_tokens": 12, "output_tokens": 3}}}),
    ];
    events.extend(text_block(0, "a"));
    events.push(json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}}));
    events.push(json!({"type": "message_stop"}));
    let decoded = decode_ok(Protocol::Messages, &messages_sse(&events)).await;
    let usage = decoded
        .iter()
        .find_map(|e| match e {
            Event::Usage(u) => Some(*u),
            _ => None,
        })
        .unwrap();
    assert_eq!((usage.input, usage.output), (12, 3));
}

#[tokio::test]
async fn an_unknown_stop_reason_is_kept_verbatim() {
    let events = decode_ok(
        Protocol::Messages,
        &messages_answer(&text_block(0, "a"), "galaxy_brain"),
    )
    .await;
    assert_eq!(
        finish(&events),
        Some(&FinishReason::Other("galaxy_brain".into()))
    );
}

#[tokio::test]
async fn a_tool_call_is_announced_complete_only_after_its_block_closes() {
    let content = tool_block(0, "t1", "read", &["{\"pa", "th\":\"a\"}"]);
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await;
    let end = events
        .iter()
        .position(|e| matches!(e, Event::ToolCallEnd { .. }))
        .unwrap();
    let last_delta = events
        .iter()
        .rposition(|e| matches!(e, Event::ToolCallDelta { .. }))
        .unwrap();
    assert!(last_delta < end);
}

#[tokio::test]
async fn malformed_final_tool_input_is_flagged_and_never_an_executable_call() {
    let content = tool_block(0, "t1", "write", &["{\"path\": \"a\", \"content\": "]);
    let events = decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await;
    let (_, _, input, malformed) = tool_ends(&events).remove(0);
    assert_eq!(input, json!({}));
    assert_eq!(
        malformed.as_deref(),
        Some("{\"path\": \"a\", \"content\": ")
    );
}

#[tokio::test]
async fn a_stream_that_fails_part_way_keeps_what_arrived_and_never_finishes() {
    let mut events = vec![message_start(1)];
    events.extend(text_block(0, "partial"));
    let canned = crate::support::Canned::body(200, &messages_sse(&events)).failing();
    let items = crate::support::collect(&canned, Protocol::Messages)
        .await
        .unwrap();
    let ok = oks(&items);
    assert_eq!(texts(&ok), "partial");
    assert!(finish(&ok).is_none());
    assert!(matches!(
        errors(&items).as_slice(),
        [LlmError::Transport(_)]
    ));
}

#[tokio::test]
async fn cache_reads_and_writes_are_reported_and_counted_once_in_the_total() {
    let events = vec![
        json!({"type": "message_start", "message": {"usage": {"input_tokens": 10, "cache_read_input_tokens": 5, "cache_creation_input_tokens": 3, "output_tokens": 1}}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 7}}),
        json!({"type": "message_stop"}),
    ];
    let decoded = decode_ok(Protocol::Messages, &messages_sse(&events)).await;
    let usage = decoded
        .iter()
        .find_map(|e| match e {
            Event::Usage(u) => Some(*u),
            _ => None,
        })
        .unwrap();
    assert_eq!((usage.cache_read, usage.cache_write), (5, 3));
    assert_eq!(usage.total(), 25);
}

#[test]
fn a_total_is_exact_for_zero_and_for_very_large_counts() {
    let zero = systemprompt_engine::llm::Usage::default();
    assert_eq!(zero.total(), 0);
    let large = systemprompt_engine::llm::Usage {
        input: 9_007_199_254_740_993,
        output: 2,
        ..zero
    };
    assert_eq!(large.total(), 9_007_199_254_740_995);
}

#[tokio::test]
#[ignore = "parity ledger: an input count the stream never states is reported as zero rather than left unknown"]
async fn a_count_the_stream_never_states_is_left_unknown() {
    let events = vec![
        json!({"type": "message_start", "message": {}}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 7}}),
        json!({"type": "message_stop"}),
    ];
    let decoded = decode_ok(Protocol::Messages, &messages_sse(&events)).await;
    assert!(
        !decoded
            .iter()
            .any(|e| matches!(e, Event::Usage(u) if u.input == 0))
    );
}
