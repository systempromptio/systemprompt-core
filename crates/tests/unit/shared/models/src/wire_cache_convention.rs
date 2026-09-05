//! Pins the cache-token convention across every outbound wire.
//!
//! `CanonicalUsage::input_tokens` is exclusive of `cache_read_tokens`, the
//! Anthropic shape. OpenAI, the Responses API and Gemini all report their
//! cached count as a subset of the prompt count, so each adapter subtracts on
//! the way in. Each wire is asserted twice: the buffered parse and the streamed
//! frame of the same reply must produce identical usage, and the two counts
//! must add back up to the prompt figure the wire actually sent.

use futures::StreamExt;
use serde_json::json;
use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalUsage, CanonicalUsageUpdate};
use systemprompt_models::wire::{anthropic, gemini, openai_chat, openai_responses};

const PROMPT: u32 = 1_000;
const CACHED: u32 = 640;
const OUTPUT: u32 = 50;

fn one_frame(sse: String) -> impl futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) })
}

fn streamed_usage(events: Vec<Result<CanonicalEvent, String>>) -> CanonicalUsage {
    let mut usage = CanonicalUsage::default();
    for event in events.into_iter().flatten() {
        match event {
            CanonicalEvent::MessageStart { usage: u, .. } => usage = u,
            CanonicalEvent::UsageDelta(update) => update.apply_to(&mut usage),
            _ => {},
        }
    }
    usage
}

fn assert_exclusive(usage: &CanonicalUsage) {
    assert_eq!(usage.cache_read_tokens, CACHED);
    assert_eq!(usage.input_tokens, PROMPT - CACHED);
    assert_eq!(usage.input_tokens + usage.cache_read_tokens, PROMPT);
}

#[tokio::test]
async fn openai_chat_buffered_excludes_the_cached_slice_from_input() {
    let value = json!({
        "id": "chatcmpl_1",
        "model": "gpt-4.1-mini",
        "usage": {
            "prompt_tokens": PROMPT,
            "completion_tokens": OUTPUT,
            "total_tokens": PROMPT + OUTPUT,
            "prompt_tokens_details": {"cached_tokens": CACHED}
        },
        "choices": [{"finish_reason": "stop", "message": {"content": "hi"}}]
    });
    assert_exclusive(&openai_chat::parse_response(&value, "fallback").usage);
}

#[tokio::test]
async fn openai_chat_buffered_and_streamed_agree_on_the_cached_slice() {
    let usage = json!({
        "prompt_tokens": PROMPT,
        "completion_tokens": OUTPUT,
        "total_tokens": PROMPT + OUTPUT,
        "prompt_tokens_details": {"cached_tokens": CACHED}
    });
    let buffered = openai_chat::parse_response(
        &json!({
            "id": "chatcmpl_1",
            "model": "gpt-4.1-mini",
            "usage": usage,
            "choices": [{"finish_reason": "stop", "message": {"content": "hi"}}]
        }),
        "fallback",
    )
    .usage;
    assert_exclusive(&buffered);

    let frame = json!({
        "id": "chatcmpl_1",
        "model": "gpt-4.1-mini",
        "choices": [],
        "usage": usage
    });
    let events = openai_chat::sse_to_canonical_events(
        one_frame(format!("data: {frame}\n\ndata: [DONE]\n\n")),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    let streamed = streamed_usage(events);
    assert_exclusive(&streamed);
    assert_eq!(streamed.input_tokens, buffered.input_tokens);
    assert_eq!(streamed.cache_read_tokens, buffered.cache_read_tokens);
    assert_eq!(streamed.total_tokens, buffered.total_tokens);
}

#[tokio::test]
async fn openai_responses_buffered_and_streamed_agree_on_the_cached_slice() {
    let usage = json!({
        "input_tokens": PROMPT,
        "output_tokens": OUTPUT,
        "total_tokens": PROMPT + OUTPUT,
        "input_tokens_details": {"cached_tokens": CACHED}
    });
    let buffered = openai_responses::parse_response_object(
        &json!({"id": "resp_1", "model": "o4-mini", "usage": usage, "output": []}),
        "fallback",
    )
    .usage;
    assert_exclusive(&buffered);

    let frame = json!({
        "type": "response.completed",
        "response": {"id": "resp_1", "model": "o4-mini", "usage": usage}
    });
    let events = openai_responses::sse_to_canonical_events(
        one_frame(format!("data: {frame}\n\n")),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    let streamed = streamed_usage(events);
    assert_exclusive(&streamed);
    assert_eq!(streamed.input_tokens, buffered.input_tokens);
    assert_eq!(streamed.cache_read_tokens, buffered.cache_read_tokens);
    assert_eq!(streamed.output_tokens, buffered.output_tokens);
}

#[tokio::test]
async fn gemini_buffered_and_streamed_agree_on_the_cached_slice() {
    let metadata = json!({
        "promptTokenCount": PROMPT,
        "candidatesTokenCount": OUTPUT,
        "cachedContentTokenCount": CACHED,
        "totalTokenCount": PROMPT + OUTPUT
    });
    let buffered = gemini::parse_response(
        &json!({
            "responseId": "msg_1",
            "modelVersion": "gemini-2.5-pro",
            "usageMetadata": metadata,
            "candidates": [{
                "finishReason": "STOP",
                "content": {"role": "model", "parts": [{"text": "hi"}]}
            }]
        }),
        "fallback",
    )
    .usage;
    assert_exclusive(&buffered);

    let frame = json!({
        "responseId": "msg_1",
        "modelVersion": "gemini-2.5-pro",
        "usageMetadata": metadata,
        "candidates": [{"finishReason": "STOP", "content": {"role": "model", "parts": [{"text": "hi"}]}}]
    });
    let events = gemini::sse_to_canonical_events(
        one_frame(format!("data: {frame}\n\n")),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    let streamed = streamed_usage(events);
    assert_exclusive(&streamed);
    assert_eq!(streamed.input_tokens, buffered.input_tokens);
    assert_eq!(streamed.cache_read_tokens, buffered.cache_read_tokens);
    assert_eq!(streamed.output_tokens, buffered.output_tokens);
}

#[tokio::test]
async fn anthropic_reports_the_two_counts_disjoint_and_is_left_alone() {
    let value = json!({
        "id": "msg_1",
        "model": "claude-sonnet-4",
        "content": [{"type": "text", "text": "hi"}],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": PROMPT - CACHED,
            "output_tokens": OUTPUT,
            "cache_read_input_tokens": CACHED,
            "cache_creation_input_tokens": 0
        }
    });
    let buffered = anthropic::parse_response(&value, "fallback").usage;
    assert_exclusive(&buffered);
    assert_eq!(buffered.total_tokens, PROMPT + OUTPUT);
}

#[test]
fn a_cached_count_larger_than_the_prompt_count_saturates_to_zero_input() {
    let value = json!({
        "id": "chatcmpl_1",
        "model": "gpt-4.1-mini",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": OUTPUT,
            "prompt_tokens_details": {"cached_tokens": 999}
        },
        "choices": []
    });
    let usage = openai_chat::parse_response(&value, "fallback").usage;
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.cache_read_tokens, 999);
}

#[test]
fn a_usage_update_without_a_cached_count_leaves_input_alone() {
    let mut usage = CanonicalUsage::default();
    CanonicalUsageUpdate {
        input_tokens: Some(PROMPT),
        output_tokens: Some(OUTPUT),
        ..CanonicalUsageUpdate::default()
    }
    .apply_to(&mut usage);
    assert_eq!(usage.input_tokens, PROMPT);
    assert_eq!(usage.cache_read_tokens, 0);
}
