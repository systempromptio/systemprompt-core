//! Unit-level coverage for the streaming-event accumulator behind the gateway
//! `stream_tap`, exercised through the `test_api` seam gated on the `test-api`
//! feature. Feeds sequences of `CanonicalEvent`s into a `TapState` and asserts
//! the accumulated snapshot and finalized `Summary` (usage, tool calls, stop
//! reason, error, final bytes, served model).

use systemprompt_api::services::gateway::protocol::{
    CanonicalContent, CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_api::services::gateway::stream_tap::test_api::{
    TapState, accumulate_event, extract_summary, snapshot,
};

fn usage(input: u32, output: u32) -> CanonicalUsage {
    CanonicalUsage {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        total_tokens: 0,
    }
}

fn feed(state: &mut TapState, events: &[CanonicalEvent]) {
    for e in events {
        accumulate_event(state, e);
    }
}

#[test]
fn accumulates_text_and_usage_and_stop() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::MessageStart {
                id: "resp_1".to_owned(),
                model: "served-model".to_owned(),
                usage: usage(10, 0),
            },
            CanonicalEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockKind::Text,
            },
            CanonicalEvent::TextDelta {
                index: 0,
                text: "Hello, ".to_owned(),
            },
            CanonicalEvent::TextDelta {
                index: 0,
                text: "world".to_owned(),
            },
            CanonicalEvent::ContentBlockStop { index: 0 },
            CanonicalEvent::UsageDelta(usage(10, 5)),
            CanonicalEvent::MessageStop {
                id: "resp_1".to_owned(),
                stop_reason: Some(CanonicalStopReason::EndTurn),
            },
        ],
    );

    let response = snapshot(&state);
    assert_eq!(response.id, "resp_1");
    assert_eq!(response.model, "served-model");
    assert_eq!(response.stop_reason, Some(CanonicalStopReason::EndTurn));
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.output_tokens, 5);
    assert_eq!(response.content.len(), 1);
    match &response.content[0] {
        CanonicalContent::Text(t) => assert_eq!(t, "Hello, world"),
        other => panic!("expected text block, got {other:?}"),
    }

    let summary = extract_summary(&mut state);
    assert_eq!(summary.usage.input_tokens, 10);
    assert_eq!(summary.usage.output_tokens, 5);
    assert!(summary.tool_calls.is_empty());
    assert!(summary.saw_stop);
    assert!(summary.error.is_none());
    assert_eq!(summary.served_model.as_deref(), Some("served-model"));
}

#[test]
fn usage_delta_ignores_zero_values() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::MessageStart {
                id: "r".to_owned(),
                model: String::new(),
                usage: usage(7, 3),
            },
            CanonicalEvent::UsageDelta(usage(0, 9)),
        ],
    );

    let response = snapshot(&state);
    assert_eq!(
        response.usage.input_tokens, 7,
        "zero input must not overwrite"
    );
    assert_eq!(response.usage.output_tokens, 9, "non-zero output updates");
}

#[test]
fn empty_message_start_model_leaves_served_model_unset() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::MessageStart {
                id: "r".to_owned(),
                model: String::new(),
                usage: usage(1, 1),
            },
            CanonicalEvent::MessageStop {
                id: "r".to_owned(),
                stop_reason: None,
            },
        ],
    );

    let summary = extract_summary(&mut state);
    assert!(summary.served_model.is_none());
    assert!(
        summary.saw_stop,
        "message_stop with no reason still records a stop (defaults to end_turn)"
    );
    let response = snapshot(&state);
    assert_eq!(
        response.stop_reason,
        Some(CanonicalStopReason::EndTurn),
        "absent stop reason defaults to end_turn"
    );
}

#[test]
fn accumulates_tool_use_block() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::MessageStart {
                id: "resp_tool".to_owned(),
                model: "m".to_owned(),
                usage: usage(5, 0),
            },
            CanonicalEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockKind::ToolUse {
                    id: "call_42".to_owned(),
                    name: "get_weather".to_owned(),
                    signature: None,
                },
            },
            CanonicalEvent::ToolUseDelta {
                index: 0,
                partial_json: "{\"city\":".to_owned(),
            },
            CanonicalEvent::ToolUseDelta {
                index: 0,
                partial_json: "\"Paris\"}".to_owned(),
            },
            CanonicalEvent::ContentBlockStop { index: 0 },
            CanonicalEvent::MessageStop {
                id: "resp_tool".to_owned(),
                stop_reason: Some(CanonicalStopReason::ToolUse),
            },
        ],
    );

    let response = snapshot(&state);
    assert_eq!(response.stop_reason, Some(CanonicalStopReason::ToolUse));
    match &response.content[0] {
        CanonicalContent::ToolUse {
            id, name, input, ..
        } => {
            assert_eq!(id, "call_42");
            assert_eq!(name, "get_weather");
            assert_eq!(input["city"], "Paris");
        },
        other => panic!("expected tool_use block, got {other:?}"),
    }

    let summary = extract_summary(&mut state);
    assert_eq!(summary.tool_calls.len(), 1);
    let call = &summary.tool_calls[0];
    assert_eq!(call.ai_tool_call_id.as_str(), "call_42");
    assert_eq!(call.tool_name, "get_weather");
    let parsed: serde_json::Value =
        serde_json::from_str(&call.tool_input).expect("tool_input is valid json");
    assert_eq!(parsed["city"], "Paris");
}

#[test]
fn malformed_tool_json_yields_empty_object() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockKind::ToolUse {
                    id: "c".to_owned(),
                    name: "fn".to_owned(),
                    signature: None,
                },
            },
            CanonicalEvent::ToolUseDelta {
                index: 0,
                partial_json: "{not json".to_owned(),
            },
        ],
    );

    let response = snapshot(&state);
    match &response.content[0] {
        CanonicalContent::ToolUse { input, .. } => {
            assert_eq!(*input, serde_json::Value::Object(serde_json::Map::new()));
        },
        other => panic!("expected tool_use block, got {other:?}"),
    }
}

#[test]
fn accumulates_thinking_with_signature() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::ContentBlockStart {
                index: 0,
                block: ContentBlockKind::Thinking { signature: None },
            },
            CanonicalEvent::ThinkingDelta {
                index: 0,
                text: "step one ".to_owned(),
            },
            CanonicalEvent::ThinkingDelta {
                index: 0,
                text: "step two".to_owned(),
            },
            CanonicalEvent::SignatureDelta {
                index: 0,
                signature: "sig-abc".to_owned(),
            },
        ],
    );

    let response = snapshot(&state);
    match &response.content[0] {
        CanonicalContent::Thinking { text, signature } => {
            assert_eq!(text, "step one step two");
            assert_eq!(signature.as_deref(), Some("sig-abc"));
        },
        other => panic!("expected thinking block, got {other:?}"),
    }
}

#[test]
fn error_event_is_recorded_in_summary() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[CanonicalEvent::Error("upstream exploded".to_owned())],
    );

    let summary = extract_summary(&mut state);
    assert_eq!(summary.error.as_deref(), Some("upstream exploded"));
    assert!(!summary.saw_stop);
    assert!(summary.tool_calls.is_empty());
}

#[test]
fn deltas_to_unknown_index_are_dropped() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[CanonicalEvent::TextDelta {
            index: 5,
            text: "orphan".to_owned(),
        }],
    );

    let response = snapshot(&state);
    assert!(
        response.content.is_empty(),
        "a delta with no prior block-start produces no content"
    );
}

#[test]
fn sparse_block_indices_backfill_with_empty_text() {
    let mut state = TapState::default();
    feed(
        &mut state,
        &[
            CanonicalEvent::ContentBlockStart {
                index: 2,
                block: ContentBlockKind::Text,
            },
            CanonicalEvent::TextDelta {
                index: 2,
                text: "third".to_owned(),
            },
        ],
    );

    let response = snapshot(&state);
    assert_eq!(response.content.len(), 3, "indices 0 and 1 backfilled");
    match &response.content[0] {
        CanonicalContent::Text(t) => assert!(t.is_empty()),
        other => panic!("expected empty text placeholder, got {other:?}"),
    }
    match &response.content[2] {
        CanonicalContent::Text(t) => assert_eq!(t, "third"),
        other => panic!("expected text block, got {other:?}"),
    }
}
