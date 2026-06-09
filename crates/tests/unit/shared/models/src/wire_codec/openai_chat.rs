//! `OpenAI` Chat Completions wire-codec tests.

use serde_json::{Value, json};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalToolChoice, ReasoningEffort, ResponseFormat, Role,
};
use systemprompt_models::wire::openai_chat;

use super::{base_request, image_url, plain_tool};

fn tool_result(id: &str, text: &str) -> CanonicalContent {
    CanonicalContent::ToolResult {
        tool_use_id: id.to_owned(),
        content: vec![CanonicalContent::Text(text.to_owned())],
        is_error: false,
        structured_content: None,
        meta: None,
    }
}

fn assistant_tool_call(id: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::ToolUse {
            id: id.to_owned(),
            name: "lookup".to_owned(),
            input: json!({"q": "rust"}),
            signature: None,
        }],
    }
}

#[test]
fn openai_chat_emits_max_completion_tokens_not_max_tokens() {
    let body = openai_chat::build_request_body(&base_request(), "upstream", None);
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
fn openai_chat_caps_reasoning_model_to_model_max_output() {
    let body = openai_chat::build_request_body(
        &base_request(),
        "gpt-5",
        Some(ModelLimits {
            max_output_tokens: 128_000,
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(128_000),
        "a reasoning model must receive the model's max_output_tokens so reasoning has budget"
    );
}

#[test]
fn openai_chat_keeps_caller_budget_for_non_reasoning_model() {
    let body = openai_chat::build_request_body(
        &base_request(),
        "gpt-4o",
        Some(ModelLimits {
            max_output_tokens: 128_000,
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(32),
        "a non-reasoning model must keep the caller's max_tokens unchanged"
    );
}

#[test]
fn openai_chat_keeps_caller_budget_when_no_model_limit() {
    let body = openai_chat::build_request_body(&base_request(), "gpt-5", None);
    assert_eq!(
        body["max_completion_tokens"],
        json!(32),
        "with no known model limit the caller's max_tokens is forwarded as-is"
    );
}

#[test]
fn openai_chat_prepends_system_message() {
    let mut req = base_request();
    req.system = Some("be terse".to_owned());
    let body = openai_chat::build_request_body(&req, "upstream", None);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "be terse");
}

#[test]
fn openai_chat_serializes_function_tools() {
    let mut req = base_request();
    req.tools = vec![plain_tool()];
    let body = openai_chat::build_request_body(&req, "upstream", None);
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
        let body = openai_chat::build_request_body(&req, "upstream", None);
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
    let body = openai_chat::build_request_body(&req, "upstream", None);
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
    let body = openai_chat::build_request_body(&req, "upstream", None);
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
    let body = openai_chat::build_request_body(&req, "upstream", None);
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
    let body = openai_chat::build_request_body(&req, "upstream", None);
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["name"], "result");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
}

#[test]
fn openai_chat_emits_tool_message_after_assistant_tool_call() {
    let mut req = base_request();
    req.messages = vec![
        assistant_tool_call("call_X"),
        CanonicalMessage {
            role: Role::User,
            content: vec![tool_result("call_X", "42")],
        },
    ];
    let body = openai_chat::build_request_body(&req, "upstream", None);
    let messages = body["messages"].as_array().expect("messages");
    // assistant tool_calls[].id immediately followed by {role:tool, tool_call_id}.
    let assistant = &messages[0];
    assert_eq!(assistant["role"], "assistant");
    assert_eq!(assistant["tool_calls"][0]["id"], "call_X");
    let tool = &messages[1];
    assert_eq!(tool["role"], "tool");
    assert_eq!(tool["tool_call_id"], "call_X");
    assert_eq!(tool["content"], "42");
    assert_eq!(
        messages.len(),
        2,
        "no stray user message for a tool-only turn"
    );
}

#[test]
fn openai_chat_emits_one_tool_message_per_result_ids_preserved() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![tool_result("call_A", "a"), tool_result("call_B", "b")],
    }];
    let body = openai_chat::build_request_body(&req, "upstream", None);
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(
        messages.len(),
        2,
        "one tool message per result, no user message"
    );
    assert_eq!(messages[0]["role"], "tool");
    assert_eq!(messages[0]["tool_call_id"], "call_A");
    assert_eq!(messages[1]["tool_call_id"], "call_B");
}

#[test]
fn openai_chat_tool_results_precede_trailing_user_text() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![
            tool_result("call_A", "a"),
            CanonicalContent::Text("and now this".to_owned()),
        ],
    }];
    let body = openai_chat::build_request_body(&req, "upstream", None);
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "tool");
    assert_eq!(messages[0]["tool_call_id"], "call_A");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "and now this");
}

#[test]
fn openai_chat_plain_user_text_still_collapses_to_string() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::Text("just text".to_owned())],
    }];
    let body = openai_chat::build_request_body(&req, "upstream", None);
    let messages = body["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "just text");
}

#[test]
fn openai_chat_clamps_non_reasoning_output_down_to_cap() {
    let mut req = base_request();
    req.max_tokens = 32_000;
    let body = openai_chat::build_request_body(
        &req,
        "zai-glm-4.7",
        Some(ModelLimits {
            max_output_tokens: 4096,
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(4096),
        "a non-reasoning model's output must be clamped down to the model-card cap"
    );
}

#[test]
fn openai_chat_clamp_never_raises_below_cap_budget() {
    let mut req = base_request();
    req.max_tokens = 1000;
    let body = openai_chat::build_request_body(
        &req,
        "zai-glm-4.7",
        Some(ModelLimits {
            max_output_tokens: 4096,
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(1000),
        "the clamp takes the min and never raises the caller's budget"
    );
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
