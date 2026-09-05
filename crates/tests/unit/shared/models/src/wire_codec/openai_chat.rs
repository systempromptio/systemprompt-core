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
    let response = openai_chat::parse_response(&value, "fallback").expect("fixture parses");
    // cached_tokens is a subset of prompt_tokens on the wire; input_tokens is
    // exclusive of it, so 12 - 5.
    assert_eq!(response.usage.input_tokens, 7);
    assert_eq!(response.usage.output_tokens, 6);
    assert_eq!(response.usage.total_tokens, 18);
    assert_eq!(response.usage.cache_read_tokens, 5);
}

#[test]
fn openai_chat_round_trips_thinking_through_reasoning_content() {
    let value: Value = json!({
        "id": "resp_think",
        "model": "qwen3-next-thinking",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "42",
                "reasoning_content": "first I counted"
            },
            "finish_reason": "stop"
        }]
    });
    let response = openai_chat::parse_response(&value, "fallback").expect("fixture parses");
    let thinking = response.content.iter().find_map(|c| match c {
        CanonicalContent::Thinking { text, .. } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(
        thinking.as_deref(),
        Some("first I counted"),
        "reasoning_content must arrive as Thinking, not be discarded"
    );

    let mut req = base_request();
    req.messages = vec![CanonicalMessage {
        role: Role::Assistant,
        content: response.content.clone(),
    }];
    let body = openai_chat::build_request_body(&req, "upstream", None);
    let assistant = &body["messages"][0];
    assert_eq!(
        assistant["reasoning_content"], "first I counted",
        "the replayed turn must carry the reasoning it was given"
    );
    assert_eq!(assistant["content"], "42");
}

#[tokio::test]
async fn openai_chat_stream_does_not_overwrite_tool_calls_with_the_done_sentinel() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason};

    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\
         \"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\
         \"function\":{\"name\":\"lookup\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\
         \"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = openai_chat::sse_to_canonical_events(upstream, "m".to_owned())
        .collect()
        .await;
    let stops: Vec<_> = events
        .into_iter()
        .filter_map(|e| match e {
            Ok(CanonicalEvent::MessageStop { stop_reason, .. }) => Some(stop_reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        stops,
        vec![Some(CanonicalStopReason::ToolUse)],
        "the [DONE] sentinel must not append a second, weaker stop reason"
    );
}

// Why: the buffered parse had no finish_reason coverage at all, so nothing
// pinned `tool_calls -> ToolUse` -- the mapping every OpenAI-compatible
// upstream depends on to have its tool call executed.
#[test]
fn openai_chat_buffered_tool_calls_finish_reason_maps_to_tool_use() {
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let value: Value = json!({
        "id": "chatcmpl_1",
        "model": "gpt-x",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "lookup", "arguments": "{\"q\":\"rust\"}"}
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });
    let response = openai_chat::parse_response(&value, "fallback").expect("fixture parses");
    assert_eq!(response.stop_reason, Some(CanonicalStopReason::ToolUse));
    assert_eq!(response.raw_finish_reason.as_deref(), Some("tool_calls"));
    match response.content.first() {
        Some(CanonicalContent::ToolUse { name, input, .. }) => {
            assert_eq!(name, "lookup");
            assert_eq!(input["q"], "rust");
        },
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

// Why: several OpenAI-compatible upstreams (Cerebras, vLLM builds, Moonshot)
// send a plain `finish_reason: "stop"` beside a fully-formed tool_calls array.
// Relayed verbatim the client ends the turn and never runs the call, and the
// drop is invisible -- it reads as the model declining to use tools.
#[test]
fn openai_chat_buffered_reports_tool_use_even_though_the_upstream_says_stop() {
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let value: Value = json!({
        "id": "chatcmpl_1",
        "model": "gpt-x",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "lookup", "arguments": "{\"q\":\"rust\"}"}
                }]
            },
            "finish_reason": "stop"
        }]
    });
    let response = openai_chat::parse_response(&value, "fallback").expect("fixture parses");
    assert_eq!(
        response.stop_reason,
        Some(CanonicalStopReason::ToolUse),
        "a turn carrying a tool call is a tool-use turn whatever the upstream calls it"
    );
    assert_eq!(
        response.raw_finish_reason.as_deref(),
        Some("stop"),
        "the wire's own reason must still be preserved verbatim for auditing"
    );
}

// Why: a call cut off mid-arguments carries unparseable JSON. Declaring tool
// use there hands the client a call it cannot run instead of telling it the
// turn was truncated.
#[test]
fn openai_chat_buffered_keeps_length_over_a_truncated_tool_call() {
    use systemprompt_models::wire::canonical::CanonicalStopReason;

    let value: Value = json!({
        "id": "chatcmpl_1",
        "model": "gpt-x",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "lookup", "arguments": "{\"q\":\"ru"}
                }]
            },
            "finish_reason": "length"
        }]
    });
    let response = openai_chat::parse_response(&value, "fallback").expect("fixture parses");
    assert_eq!(
        response.stop_reason,
        Some(CanonicalStopReason::MaxTokens),
        "truncation must survive the tool-use correction"
    );
}

#[tokio::test]
async fn openai_chat_stream_reports_tool_use_even_though_the_upstream_says_stop() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason};

    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\
         \"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\
         \"function\":{\"name\":\"lookup\",\"arguments\":\"{}\"}}]}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\
         \"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = openai_chat::sse_to_canonical_events(upstream, "m".to_owned())
        .collect()
        .await;
    let stops: Vec<_> = events
        .into_iter()
        .filter_map(|e| match e {
            Ok(CanonicalEvent::MessageStop { stop_reason, .. }) => Some(stop_reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        stops,
        vec![Some(CanonicalStopReason::ToolUse)],
        "a streamed turn that emitted tool_calls must not terminate as \"stop\""
    );
}

#[tokio::test]
async fn openai_chat_stream_keeps_length_over_a_truncated_tool_call() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::{CanonicalEvent, CanonicalStopReason};

    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\
         \"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":\
         {\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\\\"ru\"}}]}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\
         \"finish_reason\":\"length\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = openai_chat::sse_to_canonical_events(upstream, "m".to_owned())
        .collect()
        .await;
    let stops: Vec<_> = events
        .into_iter()
        .filter_map(|e| match e {
            Ok(CanonicalEvent::MessageStop { stop_reason, .. }) => Some(stop_reason),
            _ => None,
        })
        .collect();
    assert_eq!(
        stops,
        vec![Some(CanonicalStopReason::MaxTokens)],
        "a stream cut mid-arguments must say so, not claim a runnable tool call"
    );
}

// Why: the buffered parse reads `reasoning_content`, so a thinking model's
// trace survives a non-streamed turn. The streaming half ignored it, which
// dropped the entire output of the models whose purpose is that trace.
#[tokio::test]
async fn openai_chat_stream_carries_reasoning_content_deltas() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::CanonicalEvent;

    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\
         \"reasoning_content\":\"first I \"}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\
         \"reasoning_content\":\"counted\"}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\
         \"content\":\"42\"}}]}\n\n",
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{},\
         \"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = openai_chat::sse_to_canonical_events(upstream, "m".to_owned())
        .collect()
        .await;

    let thinking: Vec<(u32, String)> = events
        .iter()
        .filter_map(|e| match e {
            Ok(CanonicalEvent::ThinkingDelta { index, text }) => Some((*index, text.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        thinking,
        vec![(0, "first I ".to_owned()), (0, "counted".to_owned())],
        "streamed reasoning must arrive as thinking deltas on their own block"
    );

    let text_index = events.iter().find_map(|e| match e {
        Ok(CanonicalEvent::TextDelta { index, .. }) => Some(*index),
        _ => None,
    });
    assert_eq!(
        text_index,
        Some(1),
        "blocks are numbered in arrival order; reasoning came first, so text follows it"
    );
    let stops = events
        .iter()
        .filter(|e| matches!(e, Ok(CanonicalEvent::ContentBlockStop { index: 0 })))
        .count();
    assert_eq!(
        stops, 1,
        "the reasoning block must be closed before the stop"
    );
}

#[test]
fn openai_chat_parse_breaks_reasoning_out_of_completion_tokens() {
    let value: Value = json!({
        "id": "resp_r",
        "model": "o4-mini",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "42"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 106,
            "total_tokens": 118,
            "completion_tokens_details": {"reasoning_tokens": 100}
        }
    });
    let response = openai_chat::parse_response(&value, "fallback").expect("fixture parses");
    assert_eq!(response.usage.reasoning_tokens, 100);
    assert_eq!(
        response.usage.output_tokens, 106,
        "the OpenAI chat contract already counts reasoning inside completion_tokens, so it \
         must be copied across untouched or every thinking turn is billed twice"
    );
    assert_eq!(response.usage.total_tokens, 118);
}

#[test]
fn openai_chat_parse_defaults_reasoning_to_zero_when_absent() {
    let value: Value = json!({
        "id": "resp_p",
        "model": "gpt-x",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}}],
        "usage": {"prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3}
    });
    assert_eq!(
        openai_chat::parse_response(&value, "fallback")
            .expect("fixture parses")
            .usage
            .reasoning_tokens,
        0
    );
}

#[tokio::test]
async fn openai_chat_stream_reports_reasoning_tokens_in_the_usage_delta() {
    use futures::StreamExt;
    use systemprompt_models::wire::canonical::CanonicalEvent;

    let sse = concat!(
        "data: {\"id\":\"c1\",\"model\":\"m\",\"choices\":[],\"usage\":{\"prompt_tokens\":9,\
         \"completion_tokens\":40,\"completion_tokens_details\":{\"reasoning_tokens\":33}}}\n\n",
        "data: [DONE]\n\n",
    );
    let upstream = futures::stream::once(async move {
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(sse.as_bytes()))
    });
    let events: Vec<_> = openai_chat::sse_to_canonical_events(upstream, "m".to_owned())
        .collect()
        .await;
    let update = events
        .into_iter()
        .find_map(|e| match e {
            Ok(CanonicalEvent::UsageDelta(u)) => Some(u),
            _ => None,
        })
        .expect("usage delta emitted");
    assert_eq!(update.reasoning_tokens, Some(33));
    assert_eq!(update.output_tokens, Some(40));
}

/// The exact response shape Vertex MaaS returns on an ordinary completion:
/// `tool_calls` and `prompt_tokens_details` present but explicitly `null`.
/// `#[serde(default)]` covers an absent field, not a null, so this failed the
/// whole `ChatCompletion` -- and `parse_response` defaulted on error, turning a
/// good answer into HTTP 200 with no content and no tokens. Nine models
/// returned blanks for it. Every assertion here is content that was lost.
#[test]
fn parse_response_survives_explicit_nulls() {
    let value: Value = json!({
        "id": "f272a569",
        "object": "chat.completion",
        "model": "qwen/qwen3-next-80b-a3b-instruct-maas",
        "choices": [{
            "index": 0,
            "finish_reason": "stop",
            "logprobs": null,
            "matched_stop": 151645,
            "message": {
                "role": "assistant",
                "content": "ok",
                "reasoning_content": null,
                "tool_calls": null
            }
        }],
        "usage": {
            "prompt_tokens": 13,
            "completion_tokens": 2,
            "total_tokens": 15,
            "prompt_tokens_details": null,
            "extra_properties": { "google": { "traffic_type": "ON_DEMAND" } }
        }
    });

    let canon = openai_chat::parse_response(&value, "fallback").expect("fixture parses");

    let text: String = canon
        .content
        .iter()
        .filter_map(|c| match c {
            CanonicalContent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "ok", "content must survive a null sibling field");
    assert_eq!(canon.usage.input_tokens, 13);
    assert_eq!(canon.usage.output_tokens, 2);
    assert_eq!(canon.usage.total_tokens, 15);
}

/// An unknown-but-populated `usage` member must not cost us the usage either:
/// Vertex adds `extra_properties`, and a strict struct would reject the object.
#[test]
fn parse_response_ignores_unknown_usage_members() {
    let value: Value = json!({
        "choices": [{"finish_reason": "stop", "message": {"content": "hi"}}],
        "usage": {
            "prompt_tokens": 7,
            "completion_tokens": 1,
            "total_tokens": 8,
            "extra_properties": { "google": { "traffic_type": "ON_DEMAND" } }
        }
    });

    let canon = openai_chat::parse_response(&value, "fallback").expect("fixture parses");

    assert_eq!(canon.usage.input_tokens, 7);
    assert_eq!(canon.usage.output_tokens, 1);
}

#[test]
fn openai_chat_treats_a_catalog_thinking_budget_as_a_reasoning_model() {
    let body = openai_chat::build_request_body(
        &base_request(),
        "qwen.qwen3-next-thinking",
        Some(ModelLimits {
            max_output_tokens: 32_768,
            max_thinking_budget: Some(8192),
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(32_768),
        "a model card carrying a thinking budget must get the model cap, whatever its name"
    );
}

#[test]
fn openai_chat_keeps_caller_budget_for_unnamed_model_without_thinking_budget() {
    let body = openai_chat::build_request_body(
        &base_request(),
        "qwen.qwen3-next-instruct",
        Some(ModelLimits {
            max_output_tokens: 32_768,
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(32),
        "no catalog thinking budget and no reasoning prefix means the caller's number stands"
    );
}

#[test]
fn openai_chat_zero_thinking_budget_is_not_a_reasoning_model() {
    let body = openai_chat::build_request_body(
        &base_request(),
        "qwen.qwen3-next-instruct",
        Some(ModelLimits {
            max_output_tokens: 32_768,
            max_thinking_budget: Some(0),
            ..Default::default()
        }),
    );
    assert_eq!(
        body["max_completion_tokens"],
        json!(32),
        "a zero budget means the model does not spend completion tokens on thought"
    );
}
