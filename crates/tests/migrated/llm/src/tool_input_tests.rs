//! Streamed tool-call arguments: assembly per call, identity, malformed and
//! incomplete input, and reading final arguments.

use serde_json::json;
use systemprompt_engine::llm::{Event, Protocol, repair_tool_input};

use crate::support::{
    chat_delta, chat_finish, chat_sse, decode_ok, messages_answer, tool_block, tool_ends,
};

#[test]
fn complete_arguments_are_read_as_given() {
    let read = repair_tool_input(r#"{"path": "a", "lines": [1, 2], "deep": {"x": null}}"#);
    assert_eq!(
        read.value,
        json!({"path": "a", "lines": [1, 2], "deep": {"x": null}})
    );
    assert!(read.malformed.is_none());
}

#[test]
#[ignore = "parity ledger: arguments with an invalid escape or a raw control character are not repaired"]
fn invalid_escapes_and_raw_control_characters_are_repaired() {
    let read = repair_tool_input("{\"text\": \"line one\nline two \\q\"}");
    assert!(read.malformed.is_none());
    assert_eq!(
        read.value["text"].as_str().map(|t| t.contains("line two")),
        Some(true)
    );
}

#[test]
#[ignore = "parity ledger: empty arguments are read as an empty object, not rejected"]
fn empty_arguments_are_rejected() {
    assert!(repair_tool_input("").malformed.is_some());
}

#[test]
fn incomplete_final_arguments_are_flagged_and_not_defaulted() {
    let read = repair_tool_input(r#"{"path": "a", "content": "unfinished"#);
    assert_eq!(
        read.malformed.as_deref(),
        Some(r#"{"path": "a", "content": "unfinished"#)
    );
    assert_eq!(read.value, json!({}));
}

#[test]
fn a_malformed_escape_in_final_arguments_is_flagged() {
    let read = repair_tool_input(r#"{"path": "a\qb"}"#);
    assert!(read.malformed.is_some());
}

#[tokio::test]
async fn one_malformed_call_does_not_lose_the_other_calls_of_its_step() {
    let mut content = tool_block(0, "good", "read", &[r#"{"path":"a"}"#]);
    content.extend(tool_block(1, "bad", "write", &[r#"{"path":"#]));
    content.extend(tool_block(2, "also-good", "read", &[r#"{"path":"b"}"#]));
    let ends =
        tool_ends(&decode_ok(Protocol::Messages, &messages_answer(&content, "tool_use")).await);
    assert_eq!(ends.len(), 3);
    assert!(ends[0].3.is_none() && ends[2].3.is_none());
    assert!(ends[1].3.is_some());
}

#[tokio::test]
async fn fragments_are_assembled_per_call_when_calls_interleave() {
    let body = chat_sse(
        &[
            chat_delta(
                &json!({"tool_calls": [{"index": 0, "id": "a", "function": {"name": "read", "arguments": "{\"path\":"}}]}),
            ),
            chat_delta(
                &json!({"tool_calls": [{"index": 1, "id": "b", "function": {"name": "grep", "arguments": "{\"pattern\":"}}]}),
            ),
            chat_delta(&json!({"tool_calls": [{"index": 0, "function": {"arguments": "\"x\"}"}}]})),
            chat_delta(&json!({"tool_calls": [{"index": 1, "function": {"arguments": "\"y\"}"}}]})),
            chat_finish("tool_calls"),
        ],
        true,
    );
    let ends = tool_ends(&decode_ok(Protocol::ChatCompletions, &body).await);
    assert_eq!(
        ends,
        vec![
            ("a".into(), "read".into(), json!({"path": "x"}), None),
            ("b".into(), "grep".into(), json!({"pattern": "y"}), None),
        ]
    );
}

#[tokio::test]
#[ignore = "parity ledger: a fragment for a call that was never started is dropped silently instead of failing"]
async fn a_fragment_for_a_call_never_started_fails() {
    let body = chat_sse(
        &[
            chat_delta(&json!({"tool_calls": [{"index": 3, "function": {"arguments": "{}"}}]})),
            chat_finish("tool_calls"),
        ],
        true,
    );
    let events = decode_ok(Protocol::ChatCompletions, &body).await;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::ToolCallEnd {
            malformed: Some(_),
            ..
        }
    )));
}
