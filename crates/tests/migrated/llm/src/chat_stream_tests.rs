//! The chat-completions stream decoder: the done sentinel, deltas, tool-call
//! buffering, finish reasons and failures.

use serde_json::json;
use systemprompt_engine::llm::{Event, FinishReason, LlmError, Protocol};

use crate::support::{
    Canned, chat_delta, chat_finish, chat_sse, collect, decode, decode_ok, errors, finish, oks,
    texts, tool_ends,
};

fn call(index: u64, id: &str, name: &str, arguments: &str) -> serde_json::Value {
    json!({"index": index, "id": id, "type": "function", "function": {"name": name, "arguments": arguments}})
}

#[tokio::test]
async fn the_stream_ends_at_the_done_sentinel_without_waiting_for_the_connection_to_close() {
    let body = chat_sse(
        &[chat_delta(&json!({"content": "hi"})), chat_finish("stop")],
        true,
    );
    let canned = Canned::body(200, &body).hanging();
    let items = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        collect(&canned, Protocol::ChatCompletions),
    )
    .await
    .expect("the stream did not end at the sentinel")
    .unwrap();
    assert_eq!(finish(&oks(&items)), Some(&FinishReason::Stop));
}

#[tokio::test]
async fn frames_after_the_done_sentinel_are_ignored() {
    let mut body = chat_sse(
        &[chat_delta(&json!({"content": "a"})), chat_finish("stop")],
        true,
    );
    body.push_str(&chat_sse(&[chat_delta(&json!({"content": "late"}))], false));
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert_eq!(texts(&events), "a");
}

#[tokio::test]
async fn bare_null_frames_between_deltas_are_ignored() {
    let body = format!(
        "data: {}\n\ndata: null\n\ndata: {}\n\n{}",
        chat_delta(&json!({"content": "a"})),
        chat_delta(&json!({"content": "b"})),
        chat_sse(&[chat_finish("stop")], true)
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert_eq!(texts(&events), "ab");
}

#[tokio::test]
async fn a_null_usage_field_is_accepted() {
    let chunk = json!({"choices": [{"index": 0, "delta": {"content": "a"}, "finish_reason": null}], "usage": null});
    let events = decode_ok(
        Protocol::ChatCompletions,
        &chat_sse(&[chunk, chat_finish("stop")], true),
    )
    .await;
    assert_eq!(texts(&events), "a");
}

#[tokio::test]
#[ignore = "parity ledger: a stream that reaches the done sentinel without any finish reason is finished as unknown, not refused"]
async fn a_stream_without_a_finish_reason_is_refused() {
    let decoded = decode(
        Protocol::ChatCompletions,
        &chat_sse(&[chat_delta(&json!({"content": "a"}))], true),
    )
    .await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: an \"end\" finish reason is kept as an unknown reason, not normalised to stop"]
async fn an_end_finish_reason_is_normalised_to_stop() {
    let events = decode_ok(
        Protocol::ChatCompletions,
        &chat_sse(
            &[chat_delta(&json!({"content": "a"})), chat_finish("end")],
            true,
        ),
    )
    .await;
    assert_eq!(finish(&events), Some(&FinishReason::Stop));
}

#[tokio::test]
#[ignore = "parity ledger: an \"error\" finish reason finishes the stream instead of failing it"]
async fn an_error_finish_reason_fails_the_stream() {
    let decoded = decode(
        Protocol::ChatCompletions,
        &chat_sse(
            &[chat_delta(&json!({"content": "a"})), chat_finish("error")],
            true,
        ),
    )
    .await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
async fn a_content_filter_finish_is_its_own_reason() {
    let events = decode_ok(
        Protocol::ChatCompletions,
        &chat_sse(
            &[
                chat_delta(&json!({"content": "a"})),
                chat_finish("content_filter"),
            ],
            true,
        ),
    )
    .await;
    assert_eq!(finish(&events), Some(&FinishReason::ContentFilter));
}

#[tokio::test]
async fn an_unknown_finish_reason_is_kept_verbatim() {
    let events = decode_ok(
        Protocol::ChatCompletions,
        &chat_sse(
            &[
                chat_delta(&json!({"content": "a"})),
                chat_finish("upstream_specific"),
            ],
            true,
        ),
    )
    .await;
    assert_eq!(
        finish(&events),
        Some(&FinishReason::Other("upstream_specific".into()))
    );
}

#[tokio::test]
#[ignore = "parity ledger: a tool-call change that arrives after the finish chunk is still assembled"]
async fn a_tool_change_after_the_terminal_chunk_is_refused() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"tool_calls": [call(0, "c1", "read", "{}")]})),
            chat_finish("tool_calls"),
            chat_delta(&json!({"tool_calls": [{"index": 0, "function": {"arguments": "x"}}]})),
        ],
        true,
    );
    let decoded = decode(Protocol::ChatCompletions, &body).await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: a refusal delta is dropped instead of being shown as assistant text"]
async fn a_refusal_is_shown_as_assistant_text_joined_with_content() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"content": "I "})),
            chat_delta(&json!({"refusal": "cannot"})),
            chat_finish("stop"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert_eq!(texts(&events), "I cannot");
}

#[tokio::test]
async fn reasoning_after_content_has_started_is_still_delivered() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"content": "a"})),
            chat_delta(&json!({"reasoning_content": "late thought"})),
            chat_finish("stop"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert!(events.contains(&Event::ReasoningDelta {
        text: "late thought".into(),
        signature: None
    }));
}

#[tokio::test]
async fn reasoning_tokens_are_read_from_the_completion_breakdown() {
    let usage = json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 20, "completion_tokens_details": {"reasoning_tokens": 7}}});
    let events = decode_ok(
        Protocol::ChatCompletions,
        &chat_sse(
            &[
                chat_delta(&json!({"content": "a"})),
                chat_finish("stop"),
                usage,
            ],
            true,
        ),
    )
    .await;
    let usage = events
        .iter()
        .find_map(|e| match e {
            Event::Usage(u) => Some(*u),
            _ => None,
        })
        .unwrap();
    assert_eq!((usage.input, usage.output, usage.reasoning), (10, 20, 7));
}

#[tokio::test]
#[ignore = "parity ledger: tool calls are finalised even when the output was cut by the content filter"]
async fn tool_calls_are_not_finalised_when_content_is_filtered() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"tool_calls": [call(0, "c1", "read", "{\"path\":")]})),
            chat_finish("content_filter"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert!(tool_ends(&events).is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: tool calls are finalised even when the output was truncated at the length limit"]
async fn tool_calls_are_not_finalised_when_output_is_truncated() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"tool_calls": [call(0, "c1", "read", "{\"path\":")]})),
            chat_finish("length"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert!(tool_ends(&events).is_empty());
}

#[tokio::test]
async fn empty_identity_fields_on_later_deltas_keep_the_first_identity() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"tool_calls": [call(0, "c1", "read", "{\"pa")]})),
            chat_delta(
                &json!({"tool_calls": [{"index": 0, "id": "", "function": {"name": "", "arguments": "th\":\"a\"}"}}]}),
            ),
            chat_finish("tool_calls"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert_eq!(
        tool_ends(&events),
        vec![("c1".into(), "read".into(), json!({"path": "a"}), None)]
    );
}

#[tokio::test]
async fn tool_call_deltas_are_held_until_the_name_arrives() {
    let body = chat_sse(
        &[
            chat_delta(
                &json!({"tool_calls": [{"index": 0, "id": "c1", "function": {"arguments": "{\"path\":"}}]}),
            ),
            chat_delta(
                &json!({"tool_calls": [{"index": 0, "function": {"name": "read", "arguments": "\"a\"}"}}]}),
            ),
            chat_finish("tool_calls"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    let start = events
        .iter()
        .position(|e| matches!(e, Event::ToolCallStart { .. }))
        .unwrap();
    assert!(
        !events[..start]
            .iter()
            .any(|e| matches!(e, Event::ToolCallDelta { .. }))
    );
    assert_eq!(
        tool_ends(&events),
        vec![("c1".into(), "read".into(), json!({"path": "a"}), None)]
    );
}

#[tokio::test]
#[ignore = "parity ledger: a tool call that never receives a name is dropped silently instead of failing the stream"]
async fn a_tool_call_that_never_gets_a_name_fails_the_stream() {
    let body = chat_sse(
        &[
            chat_delta(
                &json!({"tool_calls": [{"index": 0, "id": "c1", "function": {"arguments": "{}"}}]}),
            ),
            chat_finish("tool_calls"),
        ],
        true,
    );
    let decoded = decode(Protocol::ChatCompletions, &body).await;
    assert!(!errors(&decoded).is_empty());
}

#[tokio::test]
async fn a_tool_call_is_finalised_when_the_stream_ends_without_a_finish_reason() {
    let body = chat_sse(
        &[chat_delta(
            &json!({"tool_calls": [call(0, "c1", "read", "{}")]}),
        )],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert_eq!(tool_ends(&events).len(), 1);
    assert_eq!(finish(&events), Some(&FinishReason::ToolCalls));
}

#[tokio::test]
async fn a_malformed_event_fails_the_stream() {
    let body = format!(
        "data: {}\n\ndata: {{oops\n\n",
        chat_delta(&json!({"content": "a"}))
    );
    let decoded = decode(Protocol::ChatCompletions, &body).await;
    assert!(matches!(errors(&decoded).as_slice(), [LlmError::Decode(_)]));
}

#[tokio::test]
async fn a_transport_failure_mid_stream_surfaces_after_what_arrived() {
    let canned = Canned::body(
        200,
        &chat_sse(&[chat_delta(&json!({"content": "part"}))], false),
    )
    .failing();
    let items = collect(&canned, Protocol::ChatCompletions).await.unwrap();
    assert_eq!(texts(&oks(&items)), "part");
    assert!(matches!(
        errors(&items).as_slice(),
        [LlmError::Transport(_)]
    ));
}

#[tokio::test]
async fn a_transport_failure_before_the_first_frame_surfaces() {
    let canned = Canned::body(200, "").failing();
    let items = collect(&canned, Protocol::ChatCompletions).await.unwrap();
    assert!(oks(&items).is_empty());
    assert!(matches!(
        errors(&items).as_slice(),
        [LlmError::Transport(_)]
    ));
}

#[tokio::test]
async fn an_error_object_mid_stream_fails_the_turn_with_a_typed_error() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"content": "a"})),
            json!({"error": {"code": "upstream_failed", "message": "the upstream gave up"}}),
        ],
        false,
    );
    let decoded = decode(Protocol::ChatCompletions, &body).await;
    match errors(&decoded).as_slice() {
        [LlmError::Provider { kind, message }] => {
            assert_eq!(kind.as_deref(), Some("upstream_failed"));
            assert_eq!(message, "the upstream gave up");
        },
        other => panic!("expected a provider error, got {other:?}"),
    }
}
