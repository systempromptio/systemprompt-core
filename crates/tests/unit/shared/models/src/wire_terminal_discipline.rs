//! Three rules every streaming codec must obey about its terminal event.
//!
//! The gateway's `stream_tap` guards the rendered output against a second
//! terminal frame, which is a correction applied downstream of the codecs. The
//! codecs themselves were never held to the same rules, so a wire that emitted
//! two `MessageStop`s, or emitted one before a tool block finished
//! accumulating, was corrected by luck rather than by contract.
//!
//! The three rules, stated once here for the two wires whose terminal signal
//! is a distinct event rather than a scalar on the last chunk:
//!
//! 1. The turn states its terminal reason exactly once. Wires restate the end
//!    of a stream -- Anthropic's `message_delta` is followed by a reason-less
//!    `message_stop` -- and what must never happen is a second, weaker reason,
//!    because an SDK that reads the last terminal frame acts on that one.
//! 2. The terminal event never precedes the tool block's completion. A client
//!    that finalises on it would run a call whose arguments are still partial.
//! 3. A turn that produced a tool call ends in `ToolUse`, whatever the wire
//!    said, because a generic stop beside a formed call is the outage this
//!    whole suite exists for.

use futures::StreamExt;
use serde_json::json;
use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason, ContentBlockKind};
use systemprompt_models::wire::{anthropic, openai_responses};

fn frames(sse: String) -> impl futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) })
}

fn is_stop(event: &CanonicalEvent) -> bool {
    matches!(event, CanonicalEvent::MessageStop { .. })
}

fn stop_reasons(events: &[CanonicalEvent]) -> Vec<CanonicalStopReason> {
    events
        .iter()
        .filter_map(|e| match e {
            CanonicalEvent::MessageStop { stop_reason, .. } => *stop_reason,
            _ => None,
        })
        .collect()
}

// Why: a terminal event that lands before the tool block's ContentBlockStop is
// rule 2 broken, and the index positions are the only way to state it.
fn assert_stop_follows_tool_block(events: &[CanonicalEvent], label: &str) {
    let first_stop = events.iter().position(is_stop);
    let block_stop = events
        .iter()
        .position(|e| matches!(e, CanonicalEvent::ContentBlockStop { .. }));
    let (Some(stop), Some(block)) = (first_stop, block_stop) else {
        panic!("{label}: expected both a tool-block stop and a terminal event; got {events:?}");
    };
    assert!(
        block < stop,
        "{label}: the turn ended before the tool block finished accumulating; got {events:?}"
    );
}

fn assert_ends_once_in_tool_use(events: &[CanonicalEvent], label: &str) {
    let stops = events.iter().filter(|e| is_stop(e)).count();
    assert_eq!(
        stops, 1,
        "{label}: the turn must end exactly once; got {events:?}"
    );
    let reasons = stop_reasons(events);
    assert_eq!(
        reasons,
        vec![CanonicalStopReason::ToolUse],
        "{label}: a turn that produced a tool call ends in ToolUse; got {events:?}"
    );
}

fn anthropic_events(frames_json: &[serde_json::Value]) -> Vec<CanonicalEvent> {
    let mut state = anthropic::AnthropicStreamState::default();
    frames_json
        .iter()
        .flat_map(|frame| state.events_from_sse(frame))
        .collect()
}

fn anthropic_tool_frames(stop_reason: &str) -> Vec<serde_json::Value> {
    vec![
        json!({
            "type": "message_start",
            "message": {"id": "msg_t", "model": "claude-sonnet-4", "usage": {"input_tokens": 5}}
        }),
        json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "tool_use", "id": "call_1", "name": "lookup", "input": {}}
        }),
        json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "input_json_delta", "partial_json": "{\"q\":\"rust\"}"}
        }),
        json!({"type": "content_block_stop", "index": 0}),
        json!({
            "type": "message_delta",
            "delta": {"stop_reason": stop_reason},
            "usage": {"output_tokens": 7}
        }),
        json!({"type": "message_stop"}),
    ]
}

#[tokio::test]
async fn anthropic_tool_turn_ends_once_in_tool_use_after_the_block_closes() {
    let events = anthropic_events(&anthropic_tool_frames("end_turn"));
    assert_stop_follows_tool_block(&events, "anthropic");
    // Why: Anthropic sends a reason-less `message_stop` after the
    // `message_delta` that carried the real reason, so the codec emits two
    // MessageStop events and only the first states a reason. Rule 1 is that
    // exactly one of them decides the turn -- a reason-less second event must
    // never introduce a weaker one.
    let reasons = stop_reasons(&events);
    assert_eq!(
        reasons,
        vec![CanonicalStopReason::ToolUse],
        "the only stated reason must be ToolUse; got {events:?}"
    );
    assert!(
        !reasons.contains(&CanonicalStopReason::EndTurn),
        "no weaker reason may follow the tool_use one; got {events:?}"
    );
}

#[tokio::test]
async fn anthropic_tool_turn_that_already_says_tool_use_is_left_alone() {
    let events = anthropic_events(&anthropic_tool_frames("tool_use"));
    assert_eq!(stop_reasons(&events), vec![CanonicalStopReason::ToolUse]);
}

// Why: rule 3's exception. A call cut off mid-arguments is not a call the
// client can run, so truncation outranks the tool-use correction.
#[tokio::test]
async fn anthropic_truncated_tool_turn_keeps_the_cutoff_reason() {
    let events = anthropic_events(&anthropic_tool_frames("max_tokens"));
    assert_eq!(stop_reasons(&events), vec![CanonicalStopReason::MaxTokens]);
}

#[tokio::test]
async fn anthropic_opens_the_tool_block_before_any_argument_delta() {
    let events = anthropic_events(&anthropic_tool_frames("end_turn"));
    let opened = events
        .iter()
        .position(|e| {
            matches!(
                e,
                CanonicalEvent::ContentBlockStart {
                    block: ContentBlockKind::ToolUse { .. },
                    ..
                }
            )
        })
        .expect("the tool block opens");
    let first_delta = events
        .iter()
        .position(|e| matches!(e, CanonicalEvent::ToolUseDelta { .. }))
        .expect("arguments stream in");
    assert!(opened < first_delta, "got {events:?}");
}

fn openai_responses_tool_sse(terminal: &str) -> String {
    [
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_t\",\"model\":\"o4-mini\",\"output\":[]}}\n\n".to_owned(),
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_1\",\"call_id\":\"call_1\",\"name\":\"lookup\",\"arguments\":\"\"}}\n\n".to_owned(),
        "data: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"delta\":\"{\\\"q\\\":\\\"rust\\\"}\"}\n\n".to_owned(),
        "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_1\",\"call_id\":\"call_1\",\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\\\"rust\\\"}\",\"status\":\"completed\"}}\n\n".to_owned(),
        terminal.to_owned(),
    ]
    .concat()
}

async fn responses_events(sse: String) -> Vec<CanonicalEvent> {
    openai_responses::sse_to_canonical_events(frames(sse), "fallback".to_owned())
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect()
}

#[tokio::test]
async fn openai_responses_tool_turn_ends_once_in_tool_use_after_the_item_is_done() {
    let terminal = "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_t\",\"model\":\"o4-mini\",\"status\":\"completed\",\"usage\":{\"input_tokens\":5,\"output_tokens\":7},\"output\":[]}}\n\n";
    let events = responses_events(openai_responses_tool_sse(terminal)).await;
    assert_stop_follows_tool_block(&events, "openai_responses");
    assert_ends_once_in_tool_use(&events, "openai_responses");
}

// Why: the Responses wire ends a stream with `response.completed` and some
// fronts follow it with a second terminal frame. The second must not add a
// terminal event of its own, or the client reads the later, weaker one.
#[tokio::test]
async fn openai_responses_repeated_terminal_frame_does_not_end_the_turn_twice() {
    let terminal = "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_t\",\"model\":\"o4-mini\",\"status\":\"completed\",\"output\":[]}}\n\n";
    let events =
        responses_events(format!("{}{terminal}", openai_responses_tool_sse(terminal))).await;
    // Why: the assertion is on the reasons, not the event count. A codec may
    // restate the end of a turn; what it may never do is state a second,
    // different reason, because a client that reads the last terminal frame
    // then acts on the weaker one and drops the call.
    let reasons = stop_reasons(&events);
    assert!(
        reasons.iter().all(|r| *r == CanonicalStopReason::ToolUse),
        "a repeated terminal frame restated a weaker reason; got {events:?}"
    );
    assert!(
        !reasons.is_empty(),
        "the turn must still state its reason; got {events:?}"
    );
}

#[tokio::test]
async fn openai_responses_truncated_tool_turn_keeps_the_cutoff_reason() {
    let terminal = "data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_t\",\"model\":\"o4-mini\",\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"output\":[]}}\n\n";
    let events = responses_events(openai_responses_tool_sse(terminal)).await;
    assert_eq!(stop_reasons(&events), vec![CanonicalStopReason::MaxTokens]);
}
