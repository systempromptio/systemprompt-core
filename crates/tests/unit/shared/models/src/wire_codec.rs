//! Tests for the canonical wire codec's expanded surface: per-dialect emission
//! of `response_format`, `reasoning_effort`, sampling penalties, and
//! server-side search, plus parsing of grounding, code-execution output, and
//! cache/total token usage.

use serde_json::{Value, json};
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, ReasoningEffort,
    ResponseFormat, Role, SearchConfig, ThinkingConfig,
};
use systemprompt_models::wire::{anthropic, gemini, openai_chat};

fn tool_with_unsupported_keywords() -> CanonicalTool {
    CanonicalTool {
        name: "do_thing".to_owned(),
        description: Some("d".to_owned()),
        input_schema: json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "additionalProperties": false,
            "propertyNames": {"pattern": "^[a-z]+$"},
            "properties": {
                "count": {"type": "integer", "exclusiveMinimum": 0}
            }
        }),
    }
}

fn base_request() -> CanonicalRequest {
    CanonicalRequest {
        model: "m".to_owned(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text("hi".to_owned())],
        }],
        max_tokens: 32,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
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
fn anthropic_json_schema_becomes_forced_structured_output_tool() {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "structured_output".to_owned(),
        schema: json!({"type": "object"}),
        strict: true,
    });
    let body = anthropic::build_request_body(&req, "upstream");
    let tools = body["tools"].as_array().expect("tools array");
    assert!(tools.iter().any(|t| t["name"] == "structured_output"));
    assert_eq!(body["tool_choice"]["type"], "tool");
    assert_eq!(body["tool_choice"]["name"], "structured_output");
}

#[test]
fn anthropic_search_turn_omits_tool_choice_and_stream() {
    let mut req = base_request();
    req.stream = true;
    req.tool_choice = Some(systemprompt_models::wire::canonical::CanonicalToolChoice::Auto);
    req.search = Some(SearchConfig {
        max_uses: Some(3),
        context_size: None,
        urls: Vec::new(),
    });
    let body = anthropic::build_request_body(&req, "upstream");
    let tools = body["tools"].as_array().expect("tools array");
    assert!(tools.iter().any(|t| t["name"] == "web_search"));
    assert!(body.get("tool_choice").is_none());
    assert!(body.get("stream").is_none());
}

#[test]
fn anthropic_parse_derives_total_and_keeps_cache_tokens() {
    let value: Value = json!({
        "id": "msg_1",
        "model": "claude-x",
        "content": [{"type": "text", "text": "ok"}],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 5,
            "cache_read_input_tokens": 4,
            "cache_creation_input_tokens": 1
        }
    });
    let response = anthropic::parse_response(&value, "fallback");
    assert_eq!(response.usage.input_tokens, 10);
    assert_eq!(response.usage.cache_read_tokens, 4);
    assert_eq!(response.usage.cache_creation_tokens, 1);
    assert_eq!(response.usage.total_tokens, 20);
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

#[test]
fn gemini_parse_surfaces_grounding_sources_and_queries() {
    let value: Value = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "answer"}]},
            "finishReason": "STOP",
            "groundingMetadata": {
                "groundingChunks": [{"web": {"uri": "https://example.com", "title": "Example"}}],
                "webSearchQueries": ["rust async"]
            }
        }],
        "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 4, "totalTokenCount": 7}
    });
    let response = gemini::parse_response(&value, "fallback");
    let grounding = response.grounding.expect("grounding present");
    assert_eq!(grounding.sources.len(), 1);
    assert_eq!(grounding.sources[0].uri, "https://example.com");
    assert_eq!(grounding.queries, vec!["rust async".to_owned()]);
    assert_eq!(response.usage.total_tokens, 7);
}

#[test]
fn gemini_tools_strip_unsupported_schema_keywords() {
    let mut req = base_request();
    req.tools = vec![tool_with_unsupported_keywords()];
    let body = gemini::build_request_body(&req, Some(24576));
    let params = &body["tools"][0]["functionDeclarations"][0]["parameters"];
    assert!(params.get("$schema").is_none(), "$schema must be stripped");
    assert!(
        params.get("additionalProperties").is_none(),
        "additionalProperties must be stripped"
    );
    assert!(
        params.get("propertyNames").is_none(),
        "propertyNames must be stripped"
    );
    assert!(
        params["properties"]["count"].get("exclusiveMinimum").is_none(),
        "exclusiveMinimum must be stripped from nested properties"
    );
    // The structural fields the parser does accept survive.
    assert_eq!(params["type"], "object");
    assert_eq!(params["properties"]["count"]["type"], "integer");
}

#[test]
fn anthropic_tools_keep_supported_keywords_but_drop_schema_metadata() {
    let mut req = base_request();
    req.tools = vec![tool_with_unsupported_keywords()];
    let body = anthropic::build_request_body(&req, "upstream");
    let schema = &body["tools"][0]["input_schema"];
    // Anthropic accepts these; only the $schema metadata is removed.
    assert!(schema.get("$schema").is_none(), "$schema metadata stripped");
    assert_eq!(schema["additionalProperties"], json!(false));
    assert!(schema.get("propertyNames").is_some());
    assert_eq!(schema["properties"]["count"]["exclusiveMinimum"], json!(0));
}

#[test]
fn gemini_clamps_thinking_budget_to_model_card_cap() {
    let mut req = base_request();
    req.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: Some(31999),
    });
    let body = gemini::build_request_body(&req, Some(24576));
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        json!(24576)
    );
}

#[test]
fn gemini_leaves_thinking_budget_unclamped_without_cap() {
    let mut req = base_request();
    req.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: Some(8192),
    });
    let body = gemini::build_request_body(&req, None);
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        json!(8192)
    );
}

#[test]
fn gemini_parse_surfaces_code_execution_output() {
    let value: Value = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [
                {"executableCode": {"language": "PYTHON", "code": "print(1)"}},
                {"codeExecutionResult": {"outcome": "OUTCOME_OK", "output": "1"}}
            ]},
            "finishReason": "STOP"
        }]
    });
    let response = gemini::parse_response(&value, "fallback");
    let exec = response.code_execution.expect("code execution present");
    assert_eq!(exec.code, "print(1)");
    assert_eq!(exec.result.as_deref(), Some("1"));
    assert_eq!(exec.outcome.as_deref(), Some("OUTCOME_OK"));
}
