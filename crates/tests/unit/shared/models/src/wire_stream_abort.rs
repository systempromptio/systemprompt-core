//! Mid-stream failures the success-shaped codecs used to drop.
//!
//! Gemini and Chat Completions both report a failure on an already-200
//! connection by sending an `{"error": ...}` object in place of a normal
//! chunk, and Gemini reports a blocked prompt as a `promptFeedback` chunk with
//! no candidate at all. Every branch of both codecs matched on the success
//! shape, so those chunks parsed as nothing and the stream simply ended: the
//! turn was audited as truncated and the caller saw a closed socket carrying
//! no reason. Each case here must reach the canonical model as an
//! [`CanonicalEvent::Error`], which is what every inbound surface renders as
//! its own error frame.

use bytes::Bytes;
use futures::StreamExt;
use systemprompt_models::wire::canonical::CanonicalEvent;
use systemprompt_models::wire::{gemini, openai_chat};

fn one_frame(sse: String) -> impl futures::Stream<Item = Result<Bytes, std::io::Error>> {
    futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(sse)) })
}

fn errors(events: &[CanonicalEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            CanonicalEvent::Error(m) => Some(m.clone()),
            _ => None,
        })
        .collect()
}

fn has_stop(events: &[CanonicalEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, CanonicalEvent::MessageStop { .. }))
}

async fn gemini_events(sse: &str) -> Vec<CanonicalEvent> {
    gemini::sse_to_canonical_events(one_frame(sse.to_owned()), "fallback".to_owned())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok event"))
        .collect()
}

async fn chat_events(sse: &str) -> Vec<CanonicalEvent> {
    openai_chat::sse_to_canonical_events(one_frame(sse.to_owned()), "fallback".to_owned())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok event"))
        .collect()
}

#[tokio::test]
async fn gemini_error_chunk_becomes_a_canonical_error() {
    let events = gemini_events(
        "data: {\"error\":{\"code\":429,\"message\":\"quota exhausted\",\"status\":\
         \"RESOURCE_EXHAUSTED\"}}\n\n",
    )
    .await;
    assert_eq!(
        errors(&events),
        vec!["quota exhausted".to_owned()],
        "a Gemini error chunk must surface, not be parsed as an empty candidate list"
    );
}

#[tokio::test]
async fn gemini_blocked_prompt_becomes_a_canonical_error() {
    let events = gemini_events("data: {\"promptFeedback\":{\"blockReason\":\"SAFETY\"}}\n\n").await;
    let messages = errors(&events);
    assert_eq!(messages.len(), 1, "expected one error; got {events:?}");
    assert!(
        messages[0].contains("SAFETY"),
        "the block reason is the only explanation the caller can get; got {messages:?}"
    );
}

#[tokio::test]
async fn gemini_normal_chunk_is_untouched() {
    let events = gemini_events(
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"hi\"}]},\
         \"finishReason\":\"STOP\"}]}\n\n",
    )
    .await;
    assert!(errors(&events).is_empty(), "got {events:?}");
    assert!(has_stop(&events), "got {events:?}");
}

#[tokio::test]
async fn chat_error_chunk_becomes_a_canonical_error() {
    let events = chat_events(
        "data: {\"error\":{\"message\":\"upstream exploded\",\"type\":\"server_error\"}}\n\n",
    )
    .await;
    assert_eq!(errors(&events), vec!["upstream exploded".to_owned()]);
}

// Why: `[DONE]` synthesises a `stop` terminal for streams that never sent a
// finish_reason, and it follows the error chunk on this wire -- so without
// suppression the failed turn was reported to the client, and audited, as a
// clean finish.
#[tokio::test]
async fn chat_error_chunk_suppresses_the_done_sentinel_stop() {
    let events =
        chat_events("data: {\"error\":{\"message\":\"upstream exploded\"}}\n\ndata: [DONE]\n\n")
            .await;
    assert_eq!(errors(&events), vec!["upstream exploded".to_owned()]);
    assert!(
        !has_stop(&events),
        "a failed turn must not also claim a terminal stop; got {events:?}"
    );
}

#[tokio::test]
async fn chat_error_without_a_message_still_surfaces() {
    let events = chat_events("data: {\"error\":{}}\n\n").await;
    assert_eq!(errors(&events), vec!["upstream error".to_owned()]);
}

// Why: `error: null` is a field every OpenAI-compatible proxy sets on healthy
// chunks, and reading it as a failure would abort every stream they serve.
#[tokio::test]
async fn chat_null_error_field_is_not_a_failure() {
    let events = chat_events(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"error\":null,\"choices\":[{\"index\":0,\
         \"delta\":{\"content\":\"hi\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
    )
    .await;
    assert!(errors(&events).is_empty(), "got {events:?}");
    assert!(has_stop(&events), "got {events:?}");
}
