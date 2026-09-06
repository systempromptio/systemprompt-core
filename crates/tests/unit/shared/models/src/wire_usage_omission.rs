//! A terminal frame that states no usage must not erase what earlier frames
//! established.
//!
//! Only the gateway's `stream_tap` accumulator was tested for accumulation, and
//! it accumulates whatever the codec emits. Nothing pinned the codecs
//! themselves: a wire whose last frame omits `usage` has to emit no usage
//! event at all, because a `CanonicalUsageUpdate` built from an absent frame
//! reads every count as a reported zero and `apply_to` writes those zeros over
//! the input and cache counts the stream already reported. The bill for a
//! large cached prompt then lands as zero input, and no test failed.
//!
//! Each case drives the wire's own streaming codec: either two frames, where
//! the first reports usage and the terminal one does not and the earlier counts
//! must survive, or a lone terminal frame with no usage, which must produce no
//! usage event at all.

use futures::StreamExt;
use serde_json::json;
use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalUsage, CanonicalUsageUpdate};
use systemprompt_models::wire::{anthropic, gemini, openai_chat, openai_responses};

const PROMPT: u32 = 2_000;
const CACHED: u32 = 1_500;
const OUTPUT: u32 = 90;

fn frames(sse: String) -> impl futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) })
}

fn accumulate(events: Vec<Result<CanonicalEvent, String>>) -> CanonicalUsage {
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

fn assert_survived(usage: &CanonicalUsage) {
    assert_eq!(
        usage.input_tokens,
        PROMPT - CACHED,
        "the terminal frame erased the input count an earlier frame reported"
    );
    assert_eq!(
        usage.cache_read_tokens, CACHED,
        "the terminal frame erased the cached count an earlier frame reported"
    );
}

#[tokio::test]
async fn openai_chat_terminal_chunk_without_usage_keeps_the_earlier_counts() {
    let with_usage = json!({
        "id": "chatcmpl_o",
        "object": "chat.completion.chunk",
        "model": "gpt-4.1-mini",
        "choices": [{"index": 0, "delta": {"content": "hi"}}],
        "usage": {
            "prompt_tokens": PROMPT,
            "completion_tokens": OUTPUT,
            "total_tokens": PROMPT + OUTPUT,
            "prompt_tokens_details": {"cached_tokens": CACHED}
        }
    });
    let terminal = json!({
        "id": "chatcmpl_o",
        "object": "chat.completion.chunk",
        "model": "gpt-4.1-mini",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });
    let events = openai_chat::sse_to_canonical_events(
        frames(format!(
            "data: {with_usage}\n\ndata: {terminal}\n\ndata: [DONE]\n\n"
        )),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    let usage = accumulate(events);
    assert_survived(&usage);
    assert_eq!(usage.output_tokens, OUTPUT);
}

// Why: the Responses wire reports usage only on its terminal frame, so there
// is no earlier frame to preserve -- the failure mode here is the mirror one.
// A terminal frame that states no usage must emit no usage event at all; an
// update synthesised from the absent object would report six zeros, and the
// tap would write them over whatever the request had already recorded.
#[tokio::test]
async fn openai_responses_completed_without_usage_emits_no_usage_event() {
    let terminal = json!({
        "type": "response.completed",
        "response": {"id": "resp_o", "model": "o4-mini", "status": "completed", "output": []}
    });
    let events = openai_responses::sse_to_canonical_events(
        frames(format!("data: {terminal}\n\n")),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    let usage_events = events
        .iter()
        .flatten()
        .filter(|e| matches!(e, CanonicalEvent::UsageDelta(_)))
        .count();
    assert_eq!(
        usage_events, 0,
        "an absent usage object is not a usage report"
    );
    assert!(
        events
            .iter()
            .flatten()
            .any(|e| matches!(e, CanonicalEvent::MessageStop { .. })),
        "the turn must still end"
    );
}

// Why: the same rule for the Chat Completions terminal chunk, which omits
// `usage` unless the caller asked for `stream_options.include_usage`.
#[tokio::test]
async fn openai_chat_chunk_without_usage_emits_no_usage_event() {
    let terminal = json!({
        "id": "chatcmpl_o",
        "object": "chat.completion.chunk",
        "model": "gpt-4.1-mini",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
    });
    let events = openai_chat::sse_to_canonical_events(
        frames(format!("data: {terminal}\n\ndata: [DONE]\n\n")),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    assert_eq!(
        events
            .iter()
            .flatten()
            .filter(|e| matches!(e, CanonicalEvent::UsageDelta(_)))
            .count(),
        0
    );
}

#[tokio::test]
async fn gemini_terminal_chunk_without_usage_metadata_keeps_the_earlier_counts() {
    let with_usage = json!({
        "responseId": "msg_g",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{"content": {"role": "model", "parts": [{"text": "hi"}]}}],
        "usageMetadata": {
            "promptTokenCount": PROMPT,
            "candidatesTokenCount": OUTPUT,
            "cachedContentTokenCount": CACHED,
            "totalTokenCount": PROMPT + OUTPUT
        }
    });
    let terminal = json!({
        "responseId": "msg_g",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{"finishReason": "STOP", "content": {"role": "model", "parts": []}}]
    });
    let events = gemini::sse_to_canonical_events(
        frames(format!("data: {with_usage}\n\ndata: {terminal}\n\n")),
        "fallback".to_owned(),
    )
    .collect::<Vec<_>>()
    .await;
    assert_survived(&accumulate(events));
}

// Why: this is the shape Anthropic actually sends. `message_start` carries the
// input and cache counts and an output of zero; `message_delta` carries the
// final `output_tokens` alone. A partial update folded in as a complete usage
// zeroes input and cache on every single Anthropic turn.
#[tokio::test]
async fn anthropic_message_delta_with_output_only_keeps_input_and_cache() {
    let mut state = anthropic::AnthropicStreamState::default();
    let start = json!({
        "type": "message_start",
        "message": {
            "id": "msg_a",
            "model": "claude-sonnet-4",
            "usage": {
                "input_tokens": PROMPT - CACHED,
                "output_tokens": 0,
                "cache_read_input_tokens": CACHED,
                "cache_creation_input_tokens": 0
            }
        }
    });
    let delta = json!({
        "type": "message_delta",
        "delta": {"stop_reason": "end_turn"},
        "usage": {"output_tokens": OUTPUT}
    });
    let stop = json!({"type": "message_stop"});

    let mut usage = CanonicalUsage::default();
    for frame in [&start, &delta, &stop] {
        for event in state.events_from_sse(frame) {
            match event {
                CanonicalEvent::MessageStart { usage: u, .. } => usage = u,
                CanonicalEvent::UsageDelta(update) => update.apply_to(&mut usage),
                _ => {},
            }
        }
    }
    assert_survived(&usage);
    assert_eq!(usage.output_tokens, OUTPUT);
    assert_eq!(
        usage.total_tokens,
        usage.billable_total(),
        "no wire total was stated, so the recomputed sum stands"
    );
}

// Why: the unit-level statement of the same rule, independent of any wire. An
// update whose fields are `None` must leave every count alone; only the
// recomputed total may move.
#[test]
fn an_empty_update_changes_no_count() {
    let mut usage = CanonicalUsage {
        input_tokens: PROMPT - CACHED,
        output_tokens: OUTPUT,
        cache_read_tokens: CACHED,
        cache_creation_tokens: 7,
        reasoning_tokens: 3,
        total_tokens: 0,
    };
    let before = usage;
    CanonicalUsageUpdate::default().apply_to(&mut usage);
    assert_eq!(usage.input_tokens, before.input_tokens);
    assert_eq!(usage.output_tokens, before.output_tokens);
    assert_eq!(usage.cache_read_tokens, before.cache_read_tokens);
    assert_eq!(usage.cache_creation_tokens, before.cache_creation_tokens);
    assert_eq!(usage.reasoning_tokens, before.reasoning_tokens);
    assert_eq!(usage.total_tokens, before.billable_total());
}
