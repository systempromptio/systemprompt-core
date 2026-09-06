//! A body that fails to deserialize is an error, never an empty success.
//!
//! The buffered parsers stay lenient about missing optional fields, so a
//! sparse-but-legal reply still parses. A body that breaks the wire's shape
//! does not: defaulting it produced a canonical response with no content and
//! zero usage, which the gateway relayed as a successful, unbilled turn.
//!
//! Each fixture also passes `buffered_defect` -- it carries a usage object --
//! so the parser is the only thing standing between it and a silent blank.

use serde_json::{Value, json};
use systemprompt_models::wire::{anthropic, gemini, openai_chat, openai_responses};

fn usage_present() -> Value {
    json!({"input_tokens": 3, "output_tokens": 4})
}

#[test]
fn gemini_rejects_a_candidate_missing_its_content_role() {
    let value = json!({
        "candidates": [{
            "finishReason": "STOP",
            "content": {"parts": [{"text": "hi"}]}
        }],
        "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 4}
    });

    assert!(gemini::buffered_defect(&value).is_none());
    assert!(gemini::parse_response(&value, "fallback").is_err());
}

#[test]
fn anthropic_rejects_a_content_field_that_is_not_an_array() {
    let value = json!({
        "id": "msg_1",
        "model": "claude-3",
        "content": {"type": "text", "text": "hi"},
        "usage": usage_present()
    });

    assert!(anthropic::buffered_defect(&value).is_none());
    assert!(anthropic::parse_response(&value, "fallback").is_err());
}

#[test]
fn openai_chat_rejects_a_choices_field_that_is_not_an_array() {
    let value = json!({
        "id": "chatcmpl_1",
        "model": "gpt-4.1-mini",
        "choices": "stop",
        "usage": {"prompt_tokens": 3, "completion_tokens": 4}
    });

    assert!(openai_chat::buffered_defect(&value).is_none());
    assert!(openai_chat::parse_response(&value, "fallback").is_err());
}

#[test]
fn openai_responses_rejects_an_output_field_that_is_not_an_array() {
    let value = json!({
        "id": "resp_1",
        "model": "o4-mini",
        "output": "done",
        "usage": {"input_tokens": 3, "output_tokens": 4}
    });

    assert!(openai_responses::buffered_defect(&value).is_none());
    assert!(openai_responses::parse_response_object(&value, "fallback").is_err());
}

#[test]
fn a_sparse_but_legal_body_still_parses() {
    let value = json!({"usage": usage_present()});
    let parsed = anthropic::parse_response(&value, "fallback").expect("sparse body parses");

    assert_eq!(parsed.model, "fallback");
    assert!(parsed.content.is_empty());
}
