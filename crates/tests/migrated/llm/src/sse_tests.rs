//! Event-stream framing as the model decoders see it: retry lines, null and
//! keepalive payloads, size and decode failures.

use serde_json::json;
use systemprompt_engine::llm::{LlmError, Protocol, SseParser};

use crate::support::{chat_delta, chat_finish, chat_sse, decode, decode_ok, errors, texts};

#[test]
fn a_retry_line_neither_ends_the_stream_nor_loses_neighbouring_data() {
    let mut parser = SseParser::default();
    let frames = parser.push(b"data: one\nretry: 3000\ndata: two\n\nretry: 10\n\ndata: three\n\n");
    let data: Vec<&str> = frames.iter().map(|f| f.data.as_str()).collect();
    assert_eq!(data, ["one\ntwo", "three"]);
}

#[tokio::test]
async fn bare_null_payloads_are_dropped_and_others_kept() {
    let body = format!(
        "data: null\n\n{}data: null\n\n{}",
        chat_sse(&[chat_delta(&json!({"content": "kept"}))], false),
        chat_sse(&[chat_finish("stop")], true)
    );
    assert_eq!(
        texts(&decode_ok(Protocol::ChatCompletions, &body).await),
        "kept"
    );
}

#[tokio::test]
#[ignore = "parity ledger: a keepalive comment sent as a data payload fails the stream as undecodable"]
async fn keepalive_comments_sent_as_data_are_dropped() {
    let body = format!(
        "data: : keepalive\n\n{}data: : ping\n\n{}",
        chat_sse(&[chat_delta(&json!({"content": "kept"}))], false),
        chat_sse(&[chat_finish("stop")], true)
    );
    let decoded = decode(Protocol::ChatCompletions, &body).await;
    assert!(errors(&decoded).is_empty());
}

#[tokio::test]
async fn a_payload_the_decoder_cannot_read_is_a_typed_decode_error() {
    let decoded = decode(Protocol::ChatCompletions, "data: [1,\n\n").await;
    assert!(matches!(errors(&decoded).as_slice(), [LlmError::Decode(_)]));
}
