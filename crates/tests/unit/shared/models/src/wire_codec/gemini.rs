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
fn gemini_response_schema_is_shaped_to_the_openapi_subset() {
    let mut req = base_request();
    req.response_format = Some(ResponseFormat::JsonSchema {
        name: "s".to_owned(),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "stage": {"type": ["string", "null"], "enum": ["new", "won", null]},
                "tasks": {
                    "type": "array",
                    "items": {"type": "object", "additionalProperties": false,
                              "properties": {"title": {"type": "string"}}, "required": ["title"]}
                }
            },
            "required": ["stage", "tasks"]
        }),
        strict: true,
    });
    let body = gemini::build_request_body(&req, None);
    let schema = &body["generationConfig"]["responseSchema"];
    assert!(schema.get("additionalProperties").is_none());
    assert!(
        schema["properties"]["tasks"]["items"]
            .get("additionalProperties")
            .is_none()
    );
    let stage = &schema["properties"]["stage"];
    assert_eq!(
        stage["type"], "string",
        "type list folded to its non-null type"
    );
    assert_eq!(stage["nullable"], json!(true));
    assert_eq!(stage["enum"], json!(["new", "won"]), "null leaves the enum");
    assert_eq!(schema["required"], json!(["stage", "tasks"]));
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
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
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
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
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
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
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
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
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
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
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

fn tool_result_message(
    is_error: bool,
    structured: Option<Value>,
    texts: &[&str],
) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::Tool,
        content: vec![CanonicalContent::ToolResult {
            tool_use_id: "call_1".to_owned(),
            content: texts
                .iter()
                .map(|t| CanonicalContent::Text((*t).to_owned()))
                .collect(),
            is_error,
            structured_content: structured,
            meta: None,
        }],
    }
}

#[test]
fn gemini_tool_result_error_flattens_text_into_error_payload() {
    let mut req = base_request();
    req.messages = vec![tool_result_message(true, None, &["boom", "again"])];
    let body = gemini::build_request_body(&req, None);
    let part = &body["contents"][0]["parts"][0]["functionResponse"];
    assert_eq!(part["name"], "call_1");
    assert_eq!(part["response"]["error"], "boom\nagain");
    assert_eq!(body["contents"][0]["role"], "user");
}

#[test]
fn gemini_tool_result_prefers_structured_content_over_text() {
    let mut req = base_request();
    req.messages = vec![tool_result_message(
        false,
        Some(json!({"rows": [1, 2]})),
        &["ignored text"],
    )];
    let body = gemini::build_request_body(&req, None);
    let response = &body["contents"][0]["parts"][0]["functionResponse"]["response"];
    assert_eq!(response["result"], json!({"rows": [1, 2]}));
}

#[test]
fn gemini_tool_result_without_structure_flattens_text_result() {
    let mut req = base_request();
    req.messages = vec![tool_result_message(false, None, &["line one", "line two"])];
    let body = gemini::build_request_body(&req, None);
    let response = &body["contents"][0]["parts"][0]["functionResponse"]["response"];
    assert_eq!(response["result"], "line one\nline two");
}

#[test]
fn gemini_drops_system_messages_and_replays_thinking_as_thought_parts() {
    let mut req = base_request();
    req.messages = vec![
        CanonicalMessage {
            role: Role::System,
            content: vec![CanonicalContent::Text("sys".to_owned())],
        },
        CanonicalMessage {
            role: Role::Assistant,
            content: vec![CanonicalContent::Thinking {
                text: "hidden chain".to_owned(),
                signature: Some("tsig==".to_owned()),
                id: None,
                encrypted_content: None,
            }],
        },
        CanonicalMessage {
            role: Role::Assistant,
            content: vec![CanonicalContent::Text("visible".to_owned())],
        },
    ];
    let body = gemini::build_request_body(&req, None);
    let contents = body["contents"].as_array().expect("contents array");
    assert_eq!(contents.len(), 2);
    let thought = &contents[0]["parts"][0];
    assert_eq!(thought["text"], "hidden chain");
    assert_eq!(thought["thought"], true);
    assert_eq!(thought["thoughtSignature"], "tsig==");
    assert_eq!(contents[1]["parts"][0]["text"], "visible");
}

#[test]
fn gemini_function_response_name_recovered_from_matching_tool_use() {
    let mut req = base_request();
    req.messages = vec![
        CanonicalMessage {
            role: Role::Assistant,
            content: vec![tool_use(None)],
        },
        tool_result_message(false, None, &["ok"]),
    ];
    let body = gemini::build_request_body(&req, None);
    assert_eq!(
        body["contents"][1]["parts"][0]["functionResponse"]["name"], "lookup",
        "functionResponse.name must be the declared function name, not the minted id"
    );
}

#[test]
fn gemini_thinking_enabled_requests_thought_summaries() {
    let mut req = base_request();
    req.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: None,
    });
    let body = gemini::build_request_body(&req, None);
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["includeThoughts"],
        json!(true)
    );
}

#[test]
fn gemini_parse_maps_thought_parts_to_thinking_with_signature() {
    let value = json!({
        "candidates": [{ "content": { "role": "model", "parts": [
            { "text": "let me reason", "thought": true, "thoughtSignature": "tsig==" },
            { "text": "the answer" }
        ]}, "finishReason": "STOP" }]
    });
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
    match response.content.first() {
        Some(CanonicalContent::Thinking {
            text, signature, ..
        }) => {
            assert_eq!(text, "let me reason");
            assert_eq!(signature.as_deref(), Some("tsig=="));
        },
        other => panic!("expected Thinking, got {other:?}"),
    }
    assert!(matches!(
        response.content.get(1),
        Some(CanonicalContent::Text(t)) if t == "the answer"
    ));
}

// Gemini reports finishReason STOP even when the candidate it returned is a
// functionCall, so the wire's own reason cannot distinguish "finished talking"
// from "wants a tool run". Left as EndTurn it renders as `finish_reason:
// "stop"` on the OpenAI surface, and a client that follows that contract ends
// the turn without executing the tool -- the call rides along in the payload
// and is silently dropped. Measured against a live gateway: an
// OpenAI-compatible client got a tool_calls payload with finish_reason "stop"
// and ran nothing, while the same request against an Anthropic-backed model got
// "tool_calls".
#[test]
fn gemini_parse_reports_tool_use_when_the_candidate_is_a_function_call() {
    let value: Value = json!({
        "candidates": [{
            "finishReason": "STOP",
            "content": {
                "role": "model",
                "parts": [{
                    "functionCall": { "name": "systemprompt", "args": { "command": "core skills list" } }
                }]
            }
        }]
    });

    let parsed = gemini::parse_response(&value, "gemini-2.5-flash").expect("fixture parses");

    assert_eq!(
        parsed.stop_reason,
        Some(systemprompt_models::wire::canonical::CanonicalStopReason::ToolUse),
        "a functionCall candidate is a tool-use turn whatever Gemini calls it"
    );
    assert_eq!(
        parsed.raw_finish_reason.as_deref(),
        Some("STOP"),
        "the wire's own reason must still be preserved verbatim for auditing"
    );
}

#[test]
fn gemini_parse_keeps_end_turn_for_a_plain_text_candidate() {
    let value: Value = json!({
        "candidates": [{
            "finishReason": "STOP",
            "content": { "role": "model", "parts": [{ "text": "pong" }] }
        }]
    });

    let parsed = gemini::parse_response(&value, "gemini-2.5-flash").expect("fixture parses");

    assert_eq!(
        parsed.stop_reason,
        Some(systemprompt_models::wire::canonical::CanonicalStopReason::EndTurn),
        "a text-only turn must not be reported as tool use"
    );
}

// Why: the buffered path has a test for this; the streaming path had five
// tests and none about tool calls at all. Gemini reports `STOP` on the
// finishing chunk of a turn whose only part was a functionCall, so the wire's
// own reason is the wrong one and the stream state is the only signal.
#[tokio::test]
async fn gemini_stream_reports_tool_use_even_though_gemini_says_stop() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let sse = concat!(
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"functionCall\":\
         {\"name\":\"lookup\",\"args\":{\"q\":\"rust\"}}}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[]},\"finishReason\":\
         \"STOP\"}]}\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = gemini::sse_to_canonical_events(upstream, "fallback".to_owned())
        .collect()
        .await;

    let arguments = events.iter().find_map(|e| match e {
        Ok(CanonicalEvent::ToolUseDelta { partial_json, .. }) => Some(partial_json.clone()),
        _ => None,
    });
    assert_eq!(
        arguments.as_deref(),
        Some("{\"q\":\"rust\"}"),
        "the streamed call must carry its arguments"
    );

    let stop = events.into_iter().find_map(|e| match e {
        Ok(CanonicalEvent::MessageStop { stop_reason, .. }) => Some(stop_reason),
        _ => None,
    });
    assert_eq!(
        stop,
        Some(Some(CanonicalStopReason::ToolUse)),
        "STOP on a functionCall turn renders as finish_reason \"stop\" downstream, and the \
         client drops the call"
    );
}

// Why: the tool-use correction must not swallow truncation. Gemini reports
// MAX_TOKENS on a candidate whose functionCall was cut short; declaring tool
// use there hands the client a call whose arguments are incomplete.
#[test]
fn gemini_parse_keeps_max_tokens_over_a_truncated_function_call() {
    let value: Value = json!({
        "candidates": [{
            "finishReason": "MAX_TOKENS",
            "content": {
                "role": "model",
                "parts": [{ "functionCall": { "name": "lookup", "args": { "q": "ru" } } }]
            }
        }]
    });

    let parsed = gemini::parse_response(&value, "gemini-2.5-flash").expect("fixture parses");

    assert_eq!(
        parsed.stop_reason,
        Some(systemprompt_models::wire::canonical::CanonicalStopReason::MaxTokens),
        "a truncated turn must say so rather than claim a runnable tool call"
    );
}

#[tokio::test]
async fn gemini_stream_keeps_max_tokens_over_a_truncated_function_call() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let sse = concat!(
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"functionCall\":\
         {\"name\":\"lookup\",\"args\":{\"q\":\"ru\"}}}]}}]}\n\n",
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[]},\"finishReason\":\
         \"MAX_TOKENS\"}]}\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = gemini::sse_to_canonical_events(upstream, "fallback".to_owned())
        .collect()
        .await;

    let stop = events.into_iter().find_map(|e| match e {
        Ok(CanonicalEvent::MessageStop { stop_reason, .. }) => Some(stop_reason),
        _ => None,
    });
    assert_eq!(
        stop,
        Some(Some(CanonicalStopReason::MaxTokens)),
        "truncation must survive the tool-use correction on the streaming path too"
    );
}

#[test]
fn gemini_parse_counts_thoughts_tokens_inside_output_tokens() {
    let value: Value = json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "ok"}]},
            "finishReason": "MAX_TOKENS"
        }],
        "usageMetadata": {
            "promptTokenCount": 27,
            "candidatesTokenCount": 6,
            "thoughtsTokenCount": 194,
            "totalTokenCount": 227
        }
    });
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
    assert_eq!(response.usage.reasoning_tokens, 194);
    assert_eq!(
        response.usage.output_tokens, 200,
        "thoughtsTokenCount sits beside candidatesTokenCount on the wire, so it must be \
         folded into output_tokens or the thinking spend is never billed"
    );
    assert_eq!(response.usage.total_tokens, 227);
}

#[test]
fn gemini_parse_defaults_thoughts_to_zero_when_absent() {
    let value: Value = json!({
        "candidates": [{"content": {"role": "model", "parts": [{"text": "ok"}]}}],
        "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 4, "totalTokenCount": 7}
    });
    let response = gemini::parse_response(&value, "fallback").expect("fixture parses");
    assert_eq!(response.usage.reasoning_tokens, 0);
    assert_eq!(response.usage.output_tokens, 4);
}

#[tokio::test]
async fn gemini_stream_reports_thoughts_tokens_in_the_usage_delta() {
    use futures::StreamExt;

    let frame = json!({
        "candidates": [{"content": {"role": "model", "parts": [{"text": "hi"}]}}],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 5,
            "thoughtsTokenCount": 64,
            "totalTokenCount": 79
        }
    });
    let sse = format!("data: {frame}\n\n");
    let upstream =
        futures::stream::once(async move { Ok::<_, std::io::Error>(bytes::Bytes::from(sse)) });
    let events: Vec<_> = gemini::sse_to_canonical_events(upstream, "fallback".to_owned())
        .collect()
        .await;
    let update = events
        .into_iter()
        .find_map(|e| match e {
            Ok(CanonicalEvent::UsageDelta(u)) => Some(u),
            _ => None,
        })
        .expect("usage delta emitted");
    assert_eq!(update.reasoning_tokens, Some(64));
    assert_eq!(update.output_tokens, Some(69));
}

#[test]
fn gemini_raises_the_ceiling_for_default_thinking_without_sending_a_budget() {
    let req = base_request();
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_output_tokens: 65_536,
            max_thinking_budget: Some(24_576),
            ..Default::default()
        }),
    );
    let cfg = &body["generationConfig"];
    assert!(
        cfg.get("thinkingConfig").is_none_or(Value::is_null),
        "a thinkingBudget would switch thinking on for models Google ships with it off"
    );
    assert_eq!(
        cfg["maxOutputTokens"],
        json!(24_576 + 32),
        "maxOutputTokens must leave the caller's max_tokens for visible text"
    );
}

#[test]
fn gemini_default_thinking_headroom_stays_under_the_model_output_cap() {
    let mut req = base_request();
    req.max_tokens = 4000;
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_output_tokens: 4096,
            max_thinking_budget: Some(24_576),
            ..Default::default()
        }),
    );
    let cfg = &body["generationConfig"];
    assert!(cfg.get("thinkingConfig").is_none_or(Value::is_null));
    assert_eq!(cfg["maxOutputTokens"], json!(4096));
}

#[test]
fn gemini_explicit_client_thinking_leaves_max_output_tokens_untouched() {
    let mut req = base_request();
    req.thinking = Some(ThinkingConfig {
        enabled: true,
        budget_tokens: Some(1024),
    });
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_output_tokens: 65_536,
            max_thinking_budget: Some(24_576),
            ..Default::default()
        }),
    );
    let cfg = &body["generationConfig"];
    assert_eq!(cfg["thinkingConfig"]["thinkingBudget"], json!(1024));
    assert_eq!(cfg["maxOutputTokens"], json!(32));
}

#[test]
fn gemini_model_without_thinking_budget_emits_no_thinking_config() {
    let req = base_request();
    let body = gemini::build_request_body(
        &req,
        Some(ModelLimits {
            max_output_tokens: 8192,
            ..Default::default()
        }),
    );
    let cfg = &body["generationConfig"];
    assert!(cfg.get("thinkingConfig").is_none_or(Value::is_null));
    assert_eq!(
        cfg["maxOutputTokens"],
        json!(32),
        "no catalog thinking budget means the caller's number is forwarded as-is"
    );
}
