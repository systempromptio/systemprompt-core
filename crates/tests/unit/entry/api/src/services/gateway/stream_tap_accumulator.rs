//! Unit tests for the streaming-tap accumulator: folding canonical SSE events
//! into `TapState` and extracting the audit `Summary`.

use systemprompt_api::services::gateway::protocol::canonical::CanonicalContent;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
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
        total_tokens: input + output,
    }
}

fn start(state: &mut TapState, id: &str, model: &str) {
    accumulate_event(
        state,
        &CanonicalEvent::MessageStart {
            id: id.to_owned(),
            model: model.to_owned(),
            usage: usage(10, 0),
        },
    );
}

#[test]
fn text_block_accumulates_deltas() {
    let mut state = TapState::default();
    start(&mut state, "resp-1", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::TextDelta {
            index: 0,
            text: "Hello, ".to_owned(),
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::TextDelta {
            index: 0,
            text: "world".to_owned(),
        },
    );

    let response = snapshot(&state);
    assert_eq!(response.id, "resp-1");
    assert_eq!(response.model, "model-a");
    assert_eq!(response.content.len(), 1);
    assert!(matches!(
        &response.content[0],
        CanonicalContent::Text(t) if t == "Hello, world"
    ));
}

#[test]
fn thinking_block_collects_text_and_signature() {
    let mut state = TapState::default();
    start(&mut state, "resp-2", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Thinking { signature: None },
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::ThinkingDelta {
            index: 0,
            text: "pondering".to_owned(),
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::SignatureDelta {
            index: 0,
            signature: "sig-abc".to_owned(),
        },
    );

    let response = snapshot(&state);
    assert!(matches!(
        &response.content[0],
        CanonicalContent::Thinking { text, signature }
            if text == "pondering" && signature.as_deref() == Some("sig-abc")
    ));
}

#[test]
fn tool_use_partial_json_is_parsed_when_valid() {
    let mut state = TapState::default();
    start(&mut state, "resp-3", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::ToolUse {
                id: "call-1".to_owned(),
                name: "search".to_owned(),
                signature: None,
            },
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::ToolUseDelta {
            index: 0,
            partial_json: "{\"query\":".to_owned(),
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::ToolUseDelta {
            index: 0,
            partial_json: "\"rust\"}".to_owned(),
        },
    );

    let response = snapshot(&state);
    let CanonicalContent::ToolUse {
        id, name, input, ..
    } = &response.content[0]
    else {
        panic!("expected tool use block");
    };
    assert_eq!(id, "call-1");
    assert_eq!(name, "search");
    assert_eq!(input["query"], "rust");
}

#[test]
fn tool_use_invalid_partial_json_falls_back_to_empty_object() {
    let mut state = TapState::default();
    start(&mut state, "resp-4", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::ToolUse {
                id: "call-2".to_owned(),
                name: "search".to_owned(),
                signature: None,
            },
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::ToolUseDelta {
            index: 0,
            partial_json: "{\"trunc".to_owned(),
        },
    );

    let response = snapshot(&state);
    let CanonicalContent::ToolUse { input, .. } = &response.content[0] else {
        panic!("expected tool use block");
    };
    assert_eq!(*input, serde_json::json!({}));
}

#[test]
fn block_start_at_sparse_index_pads_with_empty_text_blocks() {
    let mut state = TapState::default();
    start(&mut state, "resp-5", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 2,
            block: ContentBlockKind::Text,
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::TextDelta {
            index: 2,
            text: "third".to_owned(),
        },
    );

    let response = snapshot(&state);
    assert_eq!(response.content.len(), 3);
    assert!(matches!(&response.content[0], CanonicalContent::Text(t) if t.is_empty()));
    assert!(matches!(&response.content[2], CanonicalContent::Text(t) if t == "third"));
}

#[test]
fn deltas_for_unknown_or_mismatched_blocks_are_ignored() {
    let mut state = TapState::default();
    start(&mut state, "resp-6", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::ThinkingDelta {
            index: 0,
            text: "ignored".to_owned(),
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::TextDelta {
            index: 9,
            text: "ignored".to_owned(),
        },
    );
    accumulate_event(&mut state, &CanonicalEvent::ContentBlockStop { index: 0 });

    let response = snapshot(&state);
    assert_eq!(response.content.len(), 1);
    assert!(matches!(&response.content[0], CanonicalContent::Text(t) if t.is_empty()));
}

#[test]
fn usage_delta_updates_only_nonzero_fields() {
    let mut state = TapState::default();
    start(&mut state, "resp-7", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::UsageDelta(CanonicalUsage {
            input_tokens: 0,
            output_tokens: 42,
            cache_read_tokens: 7,
            cache_creation_tokens: 3,
            total_tokens: 0,
        }),
    );

    let summary = extract_summary(&mut TapState::default());
    assert_eq!(summary.usage.input_tokens, 0);

    let response = snapshot(&state);
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.output_tokens, 42);
    assert_eq!(response.usage.cache_read_tokens, 7);
    assert_eq!(response.usage.cache_creation_tokens, 3);
}

#[test]
fn message_stop_without_reason_defaults_to_end_turn() {
    let mut state = TapState::default();
    start(&mut state, "resp-8", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::MessageStop {
            id: "resp-8".to_owned(),
            stop_reason: None,
        },
    );

    let response = snapshot(&state);
    assert_eq!(response.stop_reason, Some(CanonicalStopReason::EndTurn));
}

#[test]
fn message_start_with_empty_model_keeps_prior_model() {
    let mut state = TapState::default();
    start(&mut state, "resp-9", "model-a");
    accumulate_event(
        &mut state,
        &CanonicalEvent::MessageStart {
            id: "resp-9b".to_owned(),
            model: String::new(),
            usage: usage(1, 1),
        },
    );

    let response = snapshot(&state);
    assert_eq!(response.id, "resp-9b");
    assert_eq!(response.model, "model-a");
}

#[test]
fn extract_summary_reports_stop_error_model_and_tool_calls() {
    let mut state = TapState::default();
    start(&mut state, "resp-10", "model-b");
    accumulate_event(
        &mut state,
        &CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::ToolUse {
                id: "call-9".to_owned(),
                name: "lookup".to_owned(),
                signature: None,
            },
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::ToolUseDelta {
            index: 0,
            partial_json: "{\"k\":1}".to_owned(),
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::MessageStop {
            id: "resp-10".to_owned(),
            stop_reason: Some(CanonicalStopReason::ToolUse),
        },
    );
    accumulate_event(
        &mut state,
        &CanonicalEvent::Error("upstream hiccup".to_owned()),
    );

    let summary = extract_summary(&mut state);
    assert!(summary.saw_stop);
    assert_eq!(summary.served_model.as_deref(), Some("model-b"));
    assert_eq!(summary.error.as_deref(), Some("upstream hiccup"));
    assert_eq!(summary.usage.input_tokens, 10);
    assert_eq!(summary.tool_calls.len(), 1);
    assert_eq!(summary.tool_calls[0].tool_name, "lookup");
    assert_eq!(summary.tool_calls[0].ai_tool_call_id.as_str(), "call-9");
    assert_eq!(summary.tool_calls[0].tool_input, "{\"k\":1}");
}

#[test]
fn extract_summary_on_empty_state_has_no_model_or_stop() {
    let mut state = TapState::default();
    let summary = extract_summary(&mut state);
    assert!(!summary.saw_stop);
    assert!(summary.served_model.is_none());
    assert!(summary.error.is_none());
    assert!(summary.tool_calls.is_empty());
    assert!(summary.response.content.is_empty());
}
