//! Every buffered wire parser must refuse a body that carries no turn.
//!
//! Observed live on 2026-09-05: a Vertex `MaaS` model answered a buffered
//! `/v1/messages` call with HTTP 200 and a body the openai-chat parser
//! accepted as an empty completion. The gateway relayed a 200 with
//! `content: []`, zero tokens and no error, which a client reads as the model
//! having said nothing. `buffered_defect` is the check that runs before the
//! parser; these fixtures pin what it rejects and, just as importantly, what
//! it leaves alone.

use serde_json::{Value, json};
use systemprompt_models::wire::defect::BodyDefect;
use systemprompt_models::wire::{anthropic, gemini, openai_chat, openai_responses};

type Detector = fn(&Value) -> Option<BodyDefect>;

fn detectors() -> Vec<(&'static str, Detector)> {
    vec![
        ("openai_chat", openai_chat::buffered_defect as Detector),
        ("gemini", gemini::buffered_defect as Detector),
        (
            "openai_responses",
            openai_responses::buffered_defect as Detector,
        ),
        ("anthropic", anthropic::buffered_defect as Detector),
    ]
}

#[test]
fn an_empty_object_is_rejected_by_every_wire() {
    for (name, detect) in detectors() {
        assert_eq!(
            detect(&json!({})),
            Some(BodyDefect::NoTurn),
            "{name} accepted an empty object"
        );
    }
}

#[test]
fn a_json_array_is_rejected_by_every_wire() {
    for (name, detect) in detectors() {
        assert_eq!(
            detect(&json!([])),
            Some(BodyDefect::NotAnObject),
            "{name} accepted a JSON array"
        );
        assert_eq!(
            detect(&json!([{"error": {"message": "boom"}}])),
            Some(BodyDefect::NotAnObject),
            "{name} accepted an array of errors"
        );
    }
}

#[test]
fn an_empty_content_array_with_no_usage_is_rejected() {
    let bodies = [
        ("openai_chat", json!({"choices": []})),
        ("gemini", json!({"candidates": []})),
        ("openai_responses", json!({"output": []})),
        ("anthropic", json!({"content": []})),
    ];
    for ((name, detect), (_, body)) in detectors().into_iter().zip(bodies) {
        assert_eq!(
            detect(&body),
            Some(BodyDefect::NoTurn),
            "{name} accepted an empty content array"
        );
    }
}

#[test]
fn an_error_object_delivered_with_a_success_status_is_rejected() {
    for (name, detect) in detectors() {
        let defect = detect(&json!({"error": {"message": "model overloaded"}}));
        assert_eq!(
            defect,
            Some(BodyDefect::UpstreamErrorObject(
                "model overloaded".to_owned()
            )),
            "{name} accepted a 200-with-error body"
        );
    }
}

// Why: the point of the check is to separate "nothing came back" from "the
// model legitimately produced no text". A turn that reports usage and a stop
// reason is the second case and must still parse.
#[test]
fn a_legitimate_empty_turn_with_usage_is_accepted() {
    let bodies = [
        (
            "openai_chat",
            json!({"choices": [], "usage": {"prompt_tokens": 9, "completion_tokens": 0}}),
        ),
        (
            "gemini",
            json!({"candidates": [], "usageMetadata": {"promptTokenCount": 9}}),
        ),
        (
            "openai_responses",
            json!({"output": [], "usage": {"input_tokens": 9, "output_tokens": 0}}),
        ),
        (
            "anthropic",
            json!({"content": [], "stop_reason": "end_turn",
                   "usage": {"input_tokens": 9, "output_tokens": 0}}),
        ),
    ];
    for ((name, detect), (_, body)) in detectors().into_iter().zip(bodies) {
        assert_eq!(detect(&body), None, "{name} rejected a legitimate turn");
    }
}

#[test]
fn an_ordinary_populated_reply_is_accepted() {
    let bodies = [
        (
            "openai_chat",
            json!({"choices": [{"message": {"content": "hi"}, "finish_reason": "stop"}]}),
        ),
        (
            "gemini",
            json!({"candidates": [{"content": {"parts": [{"text": "hi"}]}}]}),
        ),
        (
            "openai_responses",
            json!({"output": [{"type": "message", "content": [{"type": "output_text",
                   "text": "hi"}]}]}),
        ),
        (
            "anthropic",
            json!({"content": [{"type": "text", "text": "hi"}]}),
        ),
    ];
    for ((name, detect), (_, body)) in detectors().into_iter().zip(bodies) {
        assert_eq!(detect(&body), None, "{name} rejected a populated reply");
    }
}
