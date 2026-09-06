//! When the Chat Completions codec states its terminal.
//!
//! The wire sends usage in a chunk of its own AFTER the chunk carrying
//! `finish_reason`, so a codec that ends the canonical turn on sight of the
//! finish reason ends it before its own counts exist. Every inbound surface
//! renders the terminal from the tap's snapshot, so the turn then reported
//! zeroed usage to the caller while the audit row -- written at the true end
//! of the stream -- carried the real numbers. The terminal is now held until
//! the stream states its end, by `[DONE]` or by ending.

use bytes::Bytes;
use futures::StreamExt;
use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason};
use systemprompt_models::wire::openai_chat;

fn one_frame(sse: String) -> impl futures::Stream<Item = Result<Bytes, std::io::Error>> {
    futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(sse)) })
}

async fn events(sse: &str) -> Vec<CanonicalEvent> {
    openai_chat::sse_to_canonical_events(one_frame(sse.to_owned()), "fallback".to_owned())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok event"))
        .collect()
}

fn position_of_stop(events: &[CanonicalEvent]) -> Option<usize> {
    events
        .iter()
        .position(|e| matches!(e, CanonicalEvent::MessageStop { .. }))
}

fn position_of_usage(events: &[CanonicalEvent]) -> Option<usize> {
    events
        .iter()
        .position(|e| matches!(e, CanonicalEvent::UsageDelta(_)))
}

const FINISH_THEN_USAGE: &str = concat!(
    "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":",
    "{\"content\":\"hi\"}}]}\n\n",
    "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},",
    "\"finish_reason\":\"stop\"}]}\n\n",
    "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[],\"usage\":",
    "{\"prompt_tokens\":11,\"completion_tokens\":18,\"total_tokens\":29}}\n\n",
    "data: [DONE]\n\n",
);

#[tokio::test]
async fn the_usage_chunk_lands_before_the_terminal() {
    let events = events(FINISH_THEN_USAGE).await;
    let usage = position_of_usage(&events).expect("the usage chunk must be read");
    let stop = position_of_stop(&events).expect("the turn must state a terminal");
    assert!(
        usage < stop,
        "the terminal must not precede the counts it reports; got {events:?}"
    );
}

#[tokio::test]
async fn the_terminal_keeps_the_reason_the_wire_stated() {
    let events = events(FINISH_THEN_USAGE).await;
    let stop = events
        .iter()
        .rev()
        .find_map(|e| match e {
            CanonicalEvent::MessageStop { stop_reason, .. } => Some(*stop_reason),
            _ => None,
        })
        .expect("terminal");
    assert_eq!(stop, Some(CanonicalStopReason::EndTurn));
}

// Why: holding the reason back means a stream that ends without the sentinel
// must still state it, or every such turn would be audited as truncated.
#[tokio::test]
async fn a_stream_that_ends_without_the_sentinel_still_stops() {
    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":",
        "{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},",
        "\"finish_reason\":\"length\"}]}\n\n",
    );
    let events = events(sse).await;
    let stop = events
        .iter()
        .rev()
        .find_map(|e| match e {
            CanonicalEvent::MessageStop { stop_reason, .. } => Some(*stop_reason),
            _ => None,
        })
        .expect("a stated finish reason must reach the terminal");
    assert_eq!(stop, Some(CanonicalStopReason::MaxTokens));
}

// Why: the terminal is stated once. `[DONE]` after a finish reason must not
// add a second, weaker one -- an SDK acts on the last terminal it reads.
#[tokio::test]
async fn the_sentinel_does_not_restate_the_terminal() {
    let events = events(FINISH_THEN_USAGE).await;
    let stops = events
        .iter()
        .filter(|e| matches!(e, CanonicalEvent::MessageStop { .. }))
        .count();
    assert_eq!(stops, 1, "got {events:?}");
}

// Why: a stream carrying no finish reason at all is a truncation, and the
// gateway reports that separately -- inventing a terminal here would hide it.
#[tokio::test]
async fn a_stream_with_no_finish_reason_states_no_terminal() {
    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":",
        "{\"content\":\"hi\"}}]}\n\n",
    );
    let events = events(sse).await;
    assert!(position_of_stop(&events).is_none(), "got {events:?}");
}

// Why: the sentinel is the only end an upstream that never states a reason
// gives, and OpenCode's turn depends on the tool-call terminal that follows.
#[tokio::test]
async fn a_tool_call_turn_states_tool_use_after_its_usage() {
    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":",
        "{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",",
        "\"function\":{\"name\":\"lookup\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},",
        "\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[],\"usage\":",
        "{\"prompt_tokens\":11,\"completion_tokens\":18}}\n\n",
        "data: [DONE]\n\n",
    );
    let events = events(sse).await;
    let usage = position_of_usage(&events).expect("usage");
    let stop = position_of_stop(&events).expect("terminal");
    assert!(usage < stop, "got {events:?}");
    assert!(matches!(
        events.last(),
        Some(CanonicalEvent::MessageStop {
            stop_reason: Some(CanonicalStopReason::ToolUse),
            ..
        })
    ));
}
