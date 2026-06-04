//! `OpenAI` Responses wire-codec tests.
//!
//! The Responses dialect differs from Chat Completions in several ways pinned
//! here: the output-token limit is `max_output_tokens`; turns go in an `input`
//! array (not `messages`); the system prompt becomes `instructions`; a system
//! turn maps to the `developer` role; reasoning is an effort bucket (derived
//! from either an explicit effort or the thinking budget); and tools are flat
//! `{type:function,name,...}` objects with no `function:{}` nesting.

use serde_json::{Value, json};
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalMessage, CanonicalToolChoice, ReasoningEffort,
    ResponseFormat, Role, SearchConfig, ThinkingConfig,
};
use systemprompt_models::wire::openai_responses;

use super::{base_request, image_url, plain_tool};

#[test]
fn openai_responses_emits_max_output_tokens_not_max_tokens() {
    let body = openai_responses::build_request_body(&base_request(), "upstream", None);
    assert_eq!(body["max_output_tokens"], json!(32));
    assert!(body.get("max_tokens").is_none());
    assert!(body.get("max_completion_tokens").is_none());
}

#[test]
fn openai_responses_caps_reasoning_model_to_model_max_output() {
    let body = openai_responses::build_request_body(&base_request(), "o3", Some(100_000));
    assert_eq!(
        body["max_output_tokens"],
        json!(100_000),
        "a reasoning model must receive the model's max_output_tokens so reasoning has budget"
    );
}

#[test]
fn openai_responses_keeps_caller_budget_for_non_reasoning_model() {
    let body = openai_responses::build_request_body(&base_request(), "gpt-4o", Some(100_000));
    assert_eq!(body["max_output_tokens"], json!(32));
}

#[test]
fn openai_responses_uses_input_array_and_instructions() {
    let mut req = base_request();
    req.system = Some("be terse".to_owned());
    let body = openai_responses::build_request_body(&req, "upstream", None);
    assert_eq!(body["instructions"], "be terse");
    assert!(body.get("messages").is_none());
    assert!(body["input"].is_array(), "input must be an array");
}

#[test]
fn openai_responses_system_message_maps_to_developer_role() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::System,
        content: vec![CanonicalContent::Text("rules".to_owned())],
    }];
    let body = openai_responses::build_request_body(&req, "upstream", None);
    assert_eq!(body["input"][0]["role"], "developer");
}

#[test]
fn openai_responses_reasoning_effort_explicit_wins() {
    let mut req = base_request();
    req.reasoning_effort = Some(ReasoningEffort::High);
    let body = openai_responses::build_request_body(&req, "upstream", None);
    assert_eq!(body["reasoning"]["effort"], "high");
}

#[test]
fn openai_responses_reasoning_effort_derived_from_thinking_budget() {
    let cases = [
        (Some(20_000), "high"),
        (Some(5_000), "medium"),
        (Some(100), "low"),
        (None, "medium"),
    ];
    for (budget, expected) in cases {
        let mut req = base_request();
        req.thinking = Some(ThinkingConfig {
            enabled: true,
            budget_tokens: budget,
        });
        let body = openai_responses::build_request_body(&req, "upstream", None);
        assert_eq!(
            body["reasoning"]["effort"], expected,
            "budget {budget:?} should bucket to {expected}"
        );
    }
}

#[test]
fn openai_responses_disabled_thinking_omits_reasoning() {
    let mut req = base_request();
    req.thinking = Some(ThinkingConfig {
        enabled: false,
        budget_tokens: Some(20_000),
    });
    let body = openai_responses::build_request_body(&req, "upstream", None);
    assert!(body.get("reasoning").is_none());
}

#[test]
fn openai_responses_serializes_flat_function_tools() {
    let mut req = base_request();
    req.tools = vec![plain_tool()];
    let body = openai_responses::build_request_body(&req, "upstream", None);
    let tool = &body["tools"][0];
    assert_eq!(tool["type"], "function");
    assert_eq!(tool["name"], "lookup");
    assert!(
        tool.get("function").is_none(),
        "Responses tools are flat, not nested under `function`"
    );
    assert_eq!(tool["parameters"]["properties"]["q"]["type"], "string");
}

#[test]
fn openai_responses_tool_choice_variants() {
    let cases = [
        (CanonicalToolChoice::Auto, json!("auto")),
        (CanonicalToolChoice::None, json!("none")),
        (CanonicalToolChoice::Required, json!("required")),
        (CanonicalToolChoice::Any, json!("required")),
        (
            CanonicalToolChoice::Tool("lookup".to_owned()),
            json!({"type": "function", "name": "lookup"}),
        ),
    ];
    for (choice, expected) in cases {
        let mut req = base_request();
        req.tools = vec![plain_tool()];
        req.tool_choice = Some(choice);
        let body = openai_responses::build_request_body(&req, "upstream", None);
        assert_eq!(body["tool_choice"], expected);
    }
}

#[test]
fn openai_responses_response_format_uses_text_format() {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "result".to_owned(),
        schema: json!({"type": "object"}),
        strict: true,
    });
    let body = openai_responses::build_request_body(&req, "upstream", None);
    assert_eq!(body["text"]["format"]["type"], "json_schema");
    assert_eq!(body["text"]["format"]["name"], "result");
}

#[test]
fn openai_responses_adds_web_search_tool() {
    let mut req = base_request();
    req.search = Some(SearchConfig {
        max_uses: None,
        context_size: Some("high".to_owned()),
        urls: Vec::new(),
    });
    let body = openai_responses::build_request_body(&req, "upstream", None);
    let tools = body["tools"].as_array().expect("tools");
    let search = tools
        .iter()
        .find(|t| t["type"] == "web_search")
        .expect("web_search tool");
    assert_eq!(search["search_context_size"], "high");
}

#[test]
fn openai_responses_renders_image_input_parts() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![image_url("https://example.com/cat.png")],
    }];
    let body = openai_responses::build_request_body(&req, "upstream", None);
    let part = &body["input"][0]["content"][0];
    assert_eq!(part["type"], "input_image");
    assert_eq!(part["image_url"], "https://example.com/cat.png");
}

#[test]
fn openai_responses_parse_extracts_text_tool_and_usage() {
    let value: Value = json!({
        "id": "resp_1",
        "model": "gpt-5.4",
        "output": [
            {"type": "message", "content": [{"type": "output_text", "text": "hello"}]},
            {"type": "function_call", "call_id": "call_1", "name": "lookup", "arguments": "{\"q\":\"rust\"}"}
        ],
        "usage": {
            "input_tokens": 10,
            "output_tokens": 4,
            "total_tokens": 14,
            "input_tokens_details": {"cached_tokens": 3}
        }
    });
    let response = openai_responses::parse_response_object(&value, "fallback");
    assert!(
        response
            .content
            .iter()
            .any(|c| matches!(c, CanonicalContent::Text(t) if t == "hello"))
    );
    assert!(response.content.iter().any(|c| matches!(
        c,
        CanonicalContent::ToolUse { name, .. } if name == "lookup"
    )));
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.output_tokens, 4);
    assert_eq!(response.usage.total_tokens, 14);
    assert_eq!(response.usage.cache_read_tokens, 3);
}

#[tokio::test]
async fn openai_responses_stream_emits_message_start() {
    use futures::StreamExt;

    let frame = json!({
        "type": "response.created",
        "response": {"id": "resp_42", "model": "gpt-5.4"}
    });
    let sse = format!("data: {frame}\n\n");
    let upstream =
        futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) });
    let events: Vec<_> = openai_responses::sse_to_canonical_events(upstream, "fallback".to_owned())
        .collect()
        .await;
    let started = events.into_iter().find_map(|e| match e {
        Ok(CanonicalEvent::MessageStart { id, .. }) => Some(id),
        _ => None,
    });
    assert_eq!(started.as_deref(), Some("resp_42"));
}
