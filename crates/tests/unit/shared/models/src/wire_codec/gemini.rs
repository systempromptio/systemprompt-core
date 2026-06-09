//! Gemini `generateContent` wire-codec tests.

use serde_json::{Value, json};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalMessage, CanonicalToolChoice, ContentBlockKind,
    ResponseFormat, Role, SearchConfig, ThinkingConfig,
};
use systemprompt_models::wire::gemini;

use super::{base_request, image_url, plain_tool, tool_use, tool_with_unsupported_keywords};

#[test]
fn gemini_request_emits_max_output_tokens_and_sampling() {
    let mut req = base_request();
    req.temperature = Some(0.5);
    req.top_p = Some(0.25);
    req.top_k = Some(40);
    let body = gemini::build_request_body(&req, None);
    let cfg = &body["generationConfig"];
    assert_eq!(cfg["maxOutputTokens"], json!(32));
    assert_eq!(cfg["temperature"], json!(0.5));
    assert_eq!(cfg["topP"], json!(0.25));
    assert_eq!(cfg["topK"], json!(40));
}

#[test]
fn gemini_clamps_max_output_tokens_down_to_model_cap() {
    let mut req = base_request();
    req.max_tokens = 32_000;
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_output_tokens: 4096,
            ..Default::default()
        }),
    );
    assert_eq!(
        body["generationConfig"]["maxOutputTokens"],
        json!(4096),
        "maxOutputTokens must be clamped down to the model-card cap when one is known"
    );
}

#[test]
fn gemini_request_emits_system_instruction() {
    let mut req = base_request();
    req.system = Some("be terse".to_owned());
    let body = gemini::build_request_body(&req, None);
    assert_eq!(body["systemInstruction"]["parts"][0]["text"], "be terse");
}

#[test]
fn gemini_tool_config_modes() {
    let cases = [
        (CanonicalToolChoice::Auto, "AUTO", None),
        (CanonicalToolChoice::None, "NONE", None),
        (CanonicalToolChoice::Required, "ANY", None),
        (CanonicalToolChoice::Any, "ANY", None),
        (
            CanonicalToolChoice::Tool("lookup".to_owned()),
            "ANY",
            Some("lookup"),
        ),
    ];
    for (choice, mode, allowed) in cases {
        let mut req = base_request();
        req.tools = vec![plain_tool()];
        req.tool_choice = Some(choice);
        let body = gemini::build_request_body(&req, None);
        let cfg = &body["toolConfig"]["functionCallingConfig"];
        assert_eq!(cfg["mode"], mode);
        match allowed {
            Some(name) => assert_eq!(cfg["allowedFunctionNames"], json!([name])),
            None => assert!(cfg.get("allowedFunctionNames").is_none()),
        }
    }
}

#[test]
fn gemini_request_adds_search_and_url_context_tools() {
    let mut req = base_request();
    req.search = Some(SearchConfig {
        max_uses: None,
        context_size: None,
        urls: vec!["https://example.com".to_owned()],
    });
    let body = gemini::build_request_body(&req, None);
    let tools = body["tools"].as_array().expect("tools");
    assert!(tools.iter().any(|t| t.get("googleSearch").is_some()));
    assert!(tools.iter().any(|t| t.get("urlContext").is_some()));
}

#[test]
fn gemini_request_adds_code_execution_tool() {
    let mut req = base_request();
    req.code_execution = true;
    let body = gemini::build_request_body(&req, None);
    let tools = body["tools"].as_array().expect("tools");
    assert!(tools.iter().any(|t| t.get("codeExecution").is_some()));
}

#[test]
fn gemini_url_image_downgraded_to_text() {
    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::User,
        content: vec![image_url("https://example.com/cat.png")],
    }];
    let body = gemini::build_request_body(&req, None);
    assert_eq!(
        body["contents"][0]["parts"][0]["text"],
        "https://example.com/cat.png"
    );
}

#[test]
fn gemini_response_format_json_schema_sets_mime_and_schema() {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "result".to_owned(),
        schema: json!({"type": "object"}),
        strict: true,
    });
    let body = gemini::build_request_body(&req, None);
    let cfg = &body["generationConfig"];
    assert_eq!(cfg["responseMimeType"], "application/json");
    assert_eq!(cfg["responseSchema"]["type"], "object");
}

#[test]
fn gemini_tools_strip_unsupported_schema_keywords() {
    let mut req = base_request();
    req.tools = vec![tool_with_unsupported_keywords()];
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_thinking_budget: Some(24576),
            ..Default::default()
        }),
    );
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
        params["properties"]["count"]
            .get("exclusiveMinimum")
            .is_none(),
        "exclusiveMinimum must be stripped from nested properties"
    );
    assert_eq!(params["type"], "object");
    assert_eq!(params["properties"]["count"]["type"], "integer");
}

#[test]
fn gemini_clamps_thinking_budget_to_model_card_cap() {
    let mut req = base_request();
    req.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: Some(31999),
    });
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_thinking_budget: Some(24576),
            ..Default::default()
        }),
    );
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
fn gemini_request_emits_thought_signature_on_function_call() {
    let mut req = base_request();
    req.messages.push(CanonicalMessage {
        role: Role::Assistant,
        content: vec![tool_use(Some("sig=="))],
    });
    let body = gemini::build_request_body(&req, None);
    let part = body["contents"]
        .as_array()
        .and_then(|c| c.iter().find(|m| m["role"] == "model"))
        .map(|m| &m["parts"][0])
        .expect("model part present");
    assert_eq!(part["functionCall"]["name"], "lookup");
    assert_eq!(part["thoughtSignature"], "sig==");
}

#[test]
fn gemini_request_omits_thought_signature_when_absent() {
    let mut req = base_request();
    req.messages.push(CanonicalMessage {
        role: Role::Assistant,
        content: vec![tool_use(None)],
    });
    let body = gemini::build_request_body(&req, None);
    let part = body["contents"]
        .as_array()
        .and_then(|c| c.iter().find(|m| m["role"] == "model"))
        .map(|m| &m["parts"][0])
        .expect("model part present");
    assert!(part.get("thoughtSignature").is_none());
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
fn gemini_parse_surfaces_cache_read_tokens() {
    let value: Value = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "ok"}]},
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 5,
            "totalTokenCount": 15,
            "cachedContentTokenCount": 6
        }
    });
    let response = gemini::parse_response(&value, "fallback");
    assert_eq!(response.usage.cache_read_tokens, 6);
    assert_eq!(response.usage.total_tokens, 15);
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

#[test]
fn gemini_parse_captures_function_call_thought_signature() {
    let value: Value = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [
                {"functionCall": {"name": "lookup", "args": {"q": "rust"}}, "thoughtSignature": "sig=="}
            ]},
            "finishReason": "STOP"
        }]
    });
    let response = gemini::parse_response(&value, "fallback");
    match response.content.first() {
        Some(CanonicalContent::ToolUse { signature, .. }) => {
            assert_eq!(signature.as_deref(), Some("sig=="));
        },
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[test]
fn gemini_parse_leaves_signature_none_when_absent() {
    let value: Value = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [
                {"functionCall": {"name": "lookup", "args": {"q": "rust"}}}
            ]},
            "finishReason": "STOP"
        }]
    });
    let response = gemini::parse_response(&value, "fallback");
    match response.content.first() {
        Some(CanonicalContent::ToolUse { signature, .. }) => assert!(signature.is_none()),
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[tokio::test]
async fn gemini_stream_emits_tool_use_block_with_signature() {
    use futures::StreamExt;

    let frame = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [
                {"functionCall": {"name": "lookup", "args": {"q": "rust"}}, "thoughtSignature": "sig=="}
            ]}
        }]
    });
    let sse = format!("data: {frame}\n\n");
    let upstream =
        futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) });
    let events: Vec<_> = gemini::sse_to_canonical_events(upstream, "fallback".to_owned())
        .collect()
        .await;
    let signature = events.into_iter().find_map(|e| match e {
        Ok(CanonicalEvent::ContentBlockStart {
            block: ContentBlockKind::ToolUse { signature, .. },
            ..
        }) => Some(signature),
        _ => None,
    });
    assert_eq!(
        signature.expect("tool-use block emitted").as_deref(),
        Some("sig==")
    );
}
