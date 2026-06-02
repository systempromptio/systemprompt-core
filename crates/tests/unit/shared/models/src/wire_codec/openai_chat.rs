//! `OpenAI` Chat Completions wire-codec tests.

use serde_json::{Value, json};
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalToolChoice, ReasoningEffort, ResponseFormat, Role,
};
use systemprompt_models::wire::openai_chat;

use super::{base_request, image_url, plain_tool};

#[test]
fn openai_chat_emits_max_completion_tokens_not_max_tokens() {
    let body = openai_chat::build_request_body(&base_request(), "upstream");
    assert_eq!(
        body["max_completion_tokens"],
        json!(32),
        "Chat Completions must use max_completion_tokens"
    );
    assert!(
        body.get("max_tokens").is_none(),
        "the deprecated max_tokens must not be emitted (gpt-5/o-series reject it)"
    );
}

#[test]
fn openai_chat_prepends_system_message() {
    let mut req = base_request();
    req.system = Some("be terse".to_owned());
    let body = openai_chat::build_request_body(&req, "upstream");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "be terse");
}

#[test]
fn openai_chat_serializes_function_tools() {
    let mut req = base_request();
    req.tools = vec![plain_tool()];
    let body = openai_chat::build_request_body(&req, "upstream");
    let tool = &body["tools"][0];
    assert_eq!(tool["type"], "function");
    assert_eq!(tool["function"]["name"], "lookup");
    assert_eq!(
        tool["function"]["parameters"]["properties"]["q"]["type"],
        "string"
    );
}

#[test]
fn openai_chat_tool_choice_variants() {
    let cases = [
        (CanonicalToolChoice::Auto, json!("auto")),
        (CanonicalToolChoice::None, json!("none")),
        (CanonicalToolChoice::Required, json!("required")),
        (CanonicalToolChoice::Any, json!("required")),
        (
            CanonicalToolChoice::Tool("lookup".to_owned()),
            json!({"type": "function", "function": {"name": "lookup"}}),
        ),
    ];
    for (choice, expected) in cases {
        let mut req = base_request();
        req.tools = vec![plain_tool()];
        req.tool_choice = Some(choice);
        let body = openai_chat::build_request_body(&req, "upstream");
        assert_eq!(body["tool_choice"], expected);
    }
}

#[test]
fn openai_chat_renders_image_url_parts() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![
            CanonicalContent::Text("look".to_owned()),
            image_url("https://example.com/cat.png"),
        ],
    }];
    let body = openai_chat::build_request_body(&req, "upstream");
    let parts = body["messages"][0]["content"].as_array().expect("parts");
    assert!(parts.iter().any(
        |p| p["type"] == "image_url" && p["image_url"]["url"] == "https://example.com/cat.png"
    ));
}

#[test]
fn openai_chat_maps_stop_sequences_and_stream_options() {
    let mut req = base_request();
    req.stop_sequences = vec!["STOP".to_owned()];
    req.stream = true;
    let body = openai_chat::build_request_body(&req, "upstream");
    assert_eq!(body["stop"], json!(["STOP"]));
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[test]
fn openai_chat_emits_penalties_and_reasoning_effort() {
    let mut req = base_request();
    req.presence_penalty = Some(0.5);
    req.frequency_penalty = Some(-0.25);
    req.reasoning_effort = Some(ReasoningEffort::Medium);
    let body = openai_chat::build_request_body(&req, "upstream");
    assert_eq!(body["presence_penalty"], json!(0.5));
    assert_eq!(body["frequency_penalty"], json!(-0.25));
    assert_eq!(body["reasoning_effort"], "medium");
}

#[test]
fn openai_chat_emits_json_schema_response_format() {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "result".to_owned(),
        schema: json!({"type": "object"}),
        strict: true,
    });
    let body = openai_chat::build_request_body(&req, "upstream");
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["name"], "result");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
}

#[test]
fn openai_chat_parse_maps_cached_and_total_tokens() {
    let value: Value = json!({
        "id": "resp_1",
        "model": "gpt-x",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "ok"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 6,
            "total_tokens": 18,
            "prompt_tokens_details": {"cached_tokens": 5}
        }
    });
    let response = openai_chat::parse_response(&value, "fallback");
    assert_eq!(response.usage.input_tokens, 12);
    assert_eq!(response.usage.output_tokens, 6);
    assert_eq!(response.usage.total_tokens, 18);
    assert_eq!(response.usage.cache_read_tokens, 5);
}
