//! Shared harness for the inbound x outbound wire matrix.
//!
//! Every other gateway test is half a pipeline: an inbound test parses a body
//! and renders a hand-built response, an outbound test builds from a hand-built
//! request and parses a canned body. Nothing joined the two, so a translation
//! that lost a tool call between the halves passed every suite — which is how
//! `3a45e861b` shipped, with Gemini's `finishReason: STOP` on a `functionCall`
//! candidate rendering as `finish_reason: "stop"` next to a fully-formed
//! `tool_calls` array. Every OpenAI-contract client read the terminal reason,
//! ended the turn, and discarded the call.
//!
//! This module drives the real `GatewayService::dispatch` against a wiremock
//! upstream for each of the three inbound surfaces crossed with each of the
//! four outbound wires. The caller's body is parsed by the real inbound
//! adapter, so the canonical request under test is the one a client would
//! actually produce. Each cell asserts two things, and the second is the one
//! that was missing everywhere: the tool call survives, AND the terminal reason
//! on the rendered response declares it.

use std::sync::Arc;

use axum::body::to_bytes;
use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::InboundAdapter;
use systemprompt_api::services::gateway::service::GatewayService;
use systemprompt_models::services::{ApiSurface, WireProtocol};
use systemprompt_test_fixtures::seed_admin_credential;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, gateway_config, gw_repos, inputs_with, install_provider_api_key,
    provider_registry,
};

/// The upstream half of one matrix cell: which wire the provider speaks, and
/// the tool-call reply it returns in that dialect.
#[derive(Debug, Clone, Copy)]
pub(super) enum OutWire {
    Anthropic,
    Gemini,
    OpenAiChat,
    OpenAiResponses,
}

impl OutWire {
    const fn wire(self) -> WireProtocol {
        match self {
            Self::Anthropic => WireProtocol::Anthropic,
            Self::Gemini => WireProtocol::Gemini,
            Self::OpenAiChat => WireProtocol::OpenAiChat,
            Self::OpenAiResponses => WireProtocol::OpenAiResponses,
        }
    }

    const fn surface(self) -> ApiSurface {
        match self {
            Self::Anthropic => ApiSurface::Anthropic,
            Self::Gemini => ApiSurface::Gemini,
            Self::OpenAiChat | Self::OpenAiResponses => ApiSurface::OpenAi,
        }
    }

    fn upstream_path(self, stream: bool) -> String {
        match self {
            Self::Anthropic => "/messages".to_owned(),
            Self::OpenAiChat => "/chat/completions".to_owned(),
            Self::OpenAiResponses => "/responses".to_owned(),
            // Why: the codec's streaming path carries `?alt=sse`, and wiremock's
            // `path` matcher compares the path alone -- passing the query with
            // it never matches and the cell fails as a connection error.
            Self::Gemini => systemprompt_models::wire::gemini::upstream_path(MODEL, stream)
                .split('?')
                .next()
                .unwrap_or_default()
                .to_owned(),
        }
    }

    fn buffered_reply(self, scenario: Scenario) -> Value {
        match (self, scenario) {
            (Self::Anthropic, Scenario::ToolCall) => anthropic_tool_reply("tool_use"),
            (Self::Anthropic, Scenario::GenericStop | Scenario::NullOptionalFields) => {
                anthropic_tool_reply("end_turn")
            },
            (Self::Anthropic, Scenario::Truncated) => anthropic_truncated_reply(),
            (
                Self::Gemini,
                Scenario::ToolCall | Scenario::GenericStop | Scenario::NullOptionalFields,
            ) => gemini_tool_reply(),
            (Self::Gemini, Scenario::Truncated) => gemini_truncated_reply(),
            (Self::OpenAiChat, Scenario::ToolCall) => openai_chat_tool_reply("tool_calls"),
            (Self::OpenAiChat, Scenario::GenericStop) => openai_chat_tool_reply("stop"),
            (Self::OpenAiChat, Scenario::NullOptionalFields) => openai_chat_null_fields_reply(),
            (Self::OpenAiChat, Scenario::Truncated) => openai_chat_truncated_reply(),
            (
                Self::OpenAiResponses,
                Scenario::ToolCall | Scenario::GenericStop | Scenario::NullOptionalFields,
            ) => openai_responses_tool_reply(),
            (Self::OpenAiResponses, Scenario::Truncated) => openai_responses_truncated_reply(),
        }
    }

    fn streaming_reply(self, scenario: Scenario) -> String {
        match (self, scenario) {
            (Self::Anthropic, Scenario::ToolCall) => anthropic_tool_sse("tool_use"),
            (Self::Anthropic, Scenario::GenericStop | Scenario::NullOptionalFields) => {
                anthropic_tool_sse("end_turn")
            },
            (Self::Anthropic, Scenario::Truncated) => anthropic_partial_tool_sse("max_tokens"),
            (
                Self::Gemini,
                Scenario::ToolCall | Scenario::GenericStop | Scenario::NullOptionalFields,
            ) => gemini_tool_sse("STOP"),
            (Self::Gemini, Scenario::Truncated) => gemini_tool_sse("MAX_TOKENS"),
            (Self::OpenAiChat, Scenario::ToolCall) => openai_chat_tool_sse("tool_calls"),
            (Self::OpenAiChat, Scenario::GenericStop) => openai_chat_tool_sse("stop"),
            (Self::OpenAiChat, Scenario::NullOptionalFields) => openai_chat_null_fields_sse(),
            (Self::OpenAiChat, Scenario::Truncated) => openai_chat_tool_sse("length"),
            (
                Self::OpenAiResponses,
                Scenario::ToolCall | Scenario::GenericStop | Scenario::NullOptionalFields,
            ) => openai_responses_tool_sse(),
            (Self::OpenAiResponses, Scenario::Truncated) => openai_responses_truncated_sse(),
        }
    }
}

/// What the upstream says about a turn that produced a tool call.
///
/// The three are one failure class seen from three angles. `ToolCall` is the
/// well-behaved upstream. `GenericStop` is the one that shipped the outage:
/// a fully-formed call under a plain "stop"/"end_turn", which every client
/// reads as a finished turn. `Truncated` is its mirror -- a call cut off
/// mid-arguments, where declaring tool use hands the client unparseable JSON
/// instead of telling it the budget ran out.
#[derive(Debug, Clone, Copy)]
pub(super) enum Scenario {
    ToolCall,
    GenericStop,
    Truncated,
    NullOptionalFields,
}

/// The tool the whole matrix exercises. Named and shaped like `plain_tool()` in
/// the wire-codec unit tests so a failure here and a failure there name the
/// same call.
pub(super) const TOOL_NAME: &str = "lookup";
pub(super) const TOOL_ARG: &str = "rust";

fn tool_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"q": {"type": "string"}}
    })
}

/// A caller body on the Anthropic Messages surface that declares the tool.
pub(super) fn anthropic_request_body(stream: bool) -> Bytes {
    body(&json!({
        "model": MODEL,
        "max_tokens": 256,
        "stream": stream,
        "messages": [{"role": "user", "content": "look up rust"}],
        "tools": [{
            "name": TOOL_NAME,
            "description": "look something up",
            "input_schema": tool_schema(),
        }],
    }))
}

/// A caller body on the `OpenAI` Chat Completions surface.
pub(super) fn openai_chat_request_body(stream: bool) -> Bytes {
    body(&json!({
        "model": MODEL,
        "max_tokens": 256,
        "stream": stream,
        "messages": [{"role": "user", "content": "look up rust"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": TOOL_NAME,
                "description": "look something up",
                "parameters": tool_schema(),
            },
        }],
    }))
}

/// A caller body on the `OpenAI` Responses surface.
pub(super) fn openai_responses_request_body(stream: bool) -> Bytes {
    body(&json!({
        "model": MODEL,
        "max_output_tokens": 256,
        "stream": stream,
        "input": [{"role": "user", "content": "look up rust"}],
        "tools": [{
            "type": "function",
            "name": TOOL_NAME,
            "description": "look something up",
            "parameters": tool_schema(),
        }],
    }))
}

fn body(value: &Value) -> Bytes {
    Bytes::from(serde_json::to_vec(value).expect("serialize caller body"))
}

fn anthropic_tool_reply(stop_reason: &str) -> Value {
    json!({
        "id": "msg_matrix",
        "type": "message",
        "role": "assistant",
        "model": MODEL,
        "content": [{
            "type": "tool_use",
            "id": "call_1",
            "name": TOOL_NAME,
            "input": {"q": TOOL_ARG},
        }],
        "stop_reason": stop_reason,
        "usage": {"input_tokens": 11, "output_tokens": 7}
    })
}

fn anthropic_truncated_reply() -> Value {
    let mut reply = anthropic_tool_reply("max_tokens");
    reply["content"][0]["input"] = json!({"q": "ru"});
    reply
}

fn gemini_truncated_reply() -> Value {
    let mut reply = gemini_tool_reply();
    reply["candidates"][0]["finishReason"] = json!("MAX_TOKENS");
    reply
}

fn openai_chat_truncated_reply() -> Value {
    let mut reply = openai_chat_tool_reply("length");
    reply["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] = json!("{\"q\":\"ru");
    reply
}

fn openai_responses_truncated_reply() -> Value {
    let mut reply = openai_responses_tool_reply();
    reply["status"] = json!("incomplete");
    reply["incomplete_details"] = json!({"reason": "max_output_tokens"});
    reply
}

// Why: `finishReason: "STOP"` on a functionCall candidate is not a typo. It is
// exactly what Gemini sends, and it is the input that broke every
// OpenAI-contract client. A fixture that "corrected" it to a tool-use reason
// would test a reply Google never sends.
fn gemini_tool_reply() -> Value {
    json!({
        "candidates": [{
            "content": {"role": "model", "parts": [
                {"functionCall": {"name": TOOL_NAME, "args": {"q": TOOL_ARG}}}
            ]},
            "finishReason": "STOP"
        }],
        "usageMetadata": {"promptTokenCount": 11, "candidatesTokenCount": 7, "totalTokenCount": 18}
    })
}

fn openai_chat_tool_reply(finish_reason: &str) -> Value {
    json!({
        "id": "chatcmpl_matrix",
        "object": "chat.completion",
        "model": MODEL,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": TOOL_NAME, "arguments": "{\"q\":\"rust\"}"},
                }],
            },
            "finish_reason": finish_reason
        }],
        "usage": {"prompt_tokens": 11, "completion_tokens": 7, "total_tokens": 18}
    })
}

fn openai_responses_tool_reply() -> Value {
    json!({
        "id": "resp_matrix",
        "object": "response",
        "status": "completed",
        "model": MODEL,
        "output": [{
            "type": "function_call",
            "id": "fc_call_1",
            "call_id": "call_1",
            "name": TOOL_NAME,
            "arguments": "{\"q\":\"rust\"}",
            "status": "completed",
        }],
        "usage": {"input_tokens": 11, "output_tokens": 7, "total_tokens": 18}
    })
}

// Why: Vertex `MaaS` and several other OpenAI-compatible fronts serialize every
// optional field rather than omitting it, so the assistant message arrives with
// `content`, `refusal`, `function_call`, `annotations` and `audio` all set to
// an explicit JSON `null` beside a fully-formed `tool_calls` array, under a
// plain `finish_reason: "stop"`. A parser that reads those keys with
// `as_str()`/`as_object()` and treats "present but null" as "malformed" drops
// the whole choice; one that only checks `is_some()` mistakes null for content.
fn openai_chat_null_fields_reply() -> Value {
    json!({
        "id": "chatcmpl_matrix_null",
        "object": "chat.completion",
        "created": 0,
        "model": MODEL,
        "system_fingerprint": Value::Null,
        "service_tier": Value::Null,
        "choices": [{
            "index": 0,
            "logprobs": Value::Null,
            "message": {
                "role": "assistant",
                "content": Value::Null,
                "refusal": Value::Null,
                "annotations": Value::Null,
                "audio": Value::Null,
                "function_call": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": TOOL_NAME, "arguments": "{\"q\":\"rust\"}"},
                }],
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 11,
            "completion_tokens": 7,
            "total_tokens": 18,
            "completion_tokens_details": Value::Null
        }
    })
}

// Why: the streaming half of the same dialect. Every chunk repeats the null
// optionals, and the terminal chunk carries `finish_reason: "stop"` with a
// null `logprobs` beside it.
fn openai_chat_null_fields_sse() -> String {
    [
        "data: {\"id\":\"chatcmpl_null\",\"object\":\"chat.completion.chunk\",\"model\":\"claude-test-model\",\"system_fingerprint\":null,\"choices\":[{\"index\":0,\"logprobs\":null,\"finish_reason\":null,\"delta\":{\"role\":\"assistant\",\"content\":null,\"refusal\":null,\"function_call\":null,\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"lookup\",\"arguments\":\"\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_null\",\"object\":\"chat.completion.chunk\",\"model\":\"claude-test-model\",\"choices\":[{\"index\":0,\"logprobs\":null,\"finish_reason\":null,\"delta\":{\"content\":null,\"refusal\":null,\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"q\\\":\\\"rust\\\"}\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_null\",\"object\":\"chat.completion.chunk\",\"model\":\"claude-test-model\",\"choices\":[{\"index\":0,\"logprobs\":null,\"delta\":{\"content\":null},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":7,\"total_tokens\":18}}\n\n",
        "data: [DONE]\n\n",
    ]
    .concat()
}

// Why: the shared `anthropic_tool_sse` streams the arguments whole, which is
// the wrong shape for a truncation cell -- an upstream that ran out of budget
// mid-call sends the opening of the JSON and then stops, with no
// `content_block_stop`. Feeding complete arguments under `max_tokens` would
// let a renderer that surfaces the call anyway still look correct.
fn anthropic_partial_tool_sse(stop_reason: &str) -> String {
    [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_m\",\"model\":\"claude-test-model\",\"usage\":{\"input_tokens\":11,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"lookup\",\"input\":{}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":\\\"ru\"}}\n\n",
        format!("event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"{stop_reason}\"}},\"usage\":{{\"output_tokens\":7}}}}\n\n").as_str(),
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]
    .concat()
}

fn anthropic_tool_sse(stop_reason: &str) -> String {
    [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_m\",\"model\":\"claude-test-model\",\"usage\":{\"input_tokens\":11,\"output_tokens\":0}}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"call_1\",\"name\":\"lookup\",\"input\":{}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":\\\"rust\\\"}\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        format!("event: message_delta\ndata: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"{stop_reason}\"}},\"usage\":{{\"output_tokens\":7}}}}\n\n").as_str(),
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]
    .concat()
}

// Why: Gemini streams a functionCall whole, in one part, and then reports
// `STOP` on the finishing chunk — the streaming half of the same lie the
// buffered fixture above tells.
fn gemini_tool_sse(finish_reason: &str) -> String {
    [
        "data: {\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"functionCall\":{\"name\":\"lookup\",\"args\":{\"q\":\"rust\"}}}]}}]}\n\n",
        format!("data: {{\"candidates\":[{{\"content\":{{\"role\":\"model\",\"parts\":[]}},\"finishReason\":\"{finish_reason}\"}}],\"usageMetadata\":{{\"promptTokenCount\":11,\"candidatesTokenCount\":7,\"totalTokenCount\":18}}}}\n\n").as_str(),
    ]
    .concat()
}

fn openai_chat_tool_sse(finish_reason: &str) -> String {
    [
        "data: {\"id\":\"chatcmpl_m\",\"object\":\"chat.completion.chunk\",\"model\":\"claude-test-model\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"lookup\",\"arguments\":\"\"}}]}}]}\n\n",
        "data: {\"id\":\"chatcmpl_m\",\"object\":\"chat.completion.chunk\",\"model\":\"claude-test-model\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"q\\\":\\\"rust\\\"}\"}}]}}]}\n\n",
        format!("data: {{\"id\":\"chatcmpl_m\",\"object\":\"chat.completion.chunk\",\"model\":\"claude-test-model\",\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"{finish_reason}\"}}],\"usage\":{{\"prompt_tokens\":11,\"completion_tokens\":7,\"total_tokens\":18}}}}\n\n").as_str(),
        "data: [DONE]\n\n",
    ]
    .concat()
}

// Why: the Responses dialect has no finish-reason field. Truncation arrives as
// a `response.incomplete` terminal event whose `incomplete_details.reason` is
// `max_output_tokens`, in place of `response.completed`.
fn openai_responses_truncated_sse() -> String {
    [
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_m\",\"model\":\"claude-test-model\",\"output\":[]}}\n\n",
        "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_call_1\",\"call_id\":\"call_1\",\"name\":\"lookup\",\"arguments\":\"\"}}\n\n",
        "event: response.function_call_arguments.delta\ndata: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"delta\":\"{\\\"q\\\":\\\"rust\"}\n\n",
        "event: response.incomplete\ndata: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"resp_m\",\"model\":\"claude-test-model\",\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{\"input_tokens\":11,\"output_tokens\":7,\"total_tokens\":18},\"output\":[]}}\n\n",
    ]
    .concat()
}

fn openai_responses_tool_sse() -> String {
    [
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_m\",\"model\":\"claude-test-model\",\"output\":[]}}\n\n",
        "event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_call_1\",\"call_id\":\"call_1\",\"name\":\"lookup\",\"arguments\":\"\"}}\n\n",
        "event: response.function_call_arguments.delta\ndata: {\"type\":\"response.function_call_arguments.delta\",\"output_index\":0,\"delta\":\"{\\\"q\\\":\\\"rust\\\"}\"}\n\n",
        "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_call_1\",\"call_id\":\"call_1\",\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\\\"rust\\\"}\",\"status\":\"completed\"}}\n\n",
        "event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_m\",\"model\":\"claude-test-model\",\"status\":\"completed\",\"usage\":{\"input_tokens\":11,\"output_tokens\":7,\"total_tokens\":18},\"output\":[]}}\n\n",
    ]
    .concat()
}

/// Runs one matrix cell end to end and returns what the caller would receive.
///
/// `label` seeds a distinct admin credential per cell; the suites run
/// concurrently against one database.
pub(super) async fn run_cell(
    label: &str,
    out: OutWire,
    inbound: Arc<dyn InboundAdapter>,
    raw: Bytes,
    stream: bool,
) -> anyhow::Result<String> {
    run_scenario(label, out, Scenario::ToolCall, inbound, raw, stream).await
}

/// Runs one matrix cell with a chosen upstream [`Scenario`].
pub(super) async fn run_scenario(
    label: &str,
    out: OutWire,
    scenario: Scenario,
    inbound: Arc<dyn InboundAdapter>,
    raw: Bytes,
    stream: bool,
) -> anyhow::Result<String> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, &format!("gw-matrix-{label}@example.invalid")).await?;

    let upstream = MockServer::start().await;
    let template = if stream {
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_raw(out.streaming_reply(scenario), "text/event-stream")
    } else {
        ResponseTemplate::new(200).set_body_json(out.buffered_reply(scenario))
    };
    Mock::given(method("POST"))
        .and(path(out.upstream_path(stream)))
        .respond_with(template)
        .mount(&upstream)
        .await;

    let request = inbound
        .parse_request(&raw)
        .map_err(|e| anyhow::anyhow!("inbound parse failed: {e}"))?;
    assert_eq!(
        request.stream, stream,
        "the caller body must ask for the lane under test"
    );
    let config = gateway_config(PROVIDER);
    let registry = provider_registry(&upstream.uri(), PROVIDER, out.wire(), out.surface());
    let di = inputs_with(&cred, request, stream, inbound, raw);

    let resp = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), di)
        .await
        .map_err(|e| anyhow::anyhow!("dispatch failed: {e:?}"))?;
    assert_eq!(resp.status(), http::StatusCode::OK, "cell {label}");
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Assertion 1: the call itself made it across, name and arguments intact.
pub(super) fn assert_tool_call_survived(label: &str, rendered: &str) {
    assert!(
        rendered.contains(TOOL_NAME),
        "{label}: the tool name did not survive the translation; body: {rendered}"
    );
    assert!(
        rendered.contains(TOOL_ARG),
        "{label}: the tool arguments did not survive the translation; body: {rendered}"
    );
}

/// Assertion 2: the terminal reason declares the tool use.
///
/// This is the assertion no cell had. A client that follows its contract reads
/// only this; a body carrying a perfect `tool_calls` array under a `"stop"`
/// finish reason is a dropped tool call, not a served one.
pub(super) fn assert_declares_tool_use(label: &str, rendered: &str, marker: &str) {
    assert!(
        rendered.contains(marker),
        "{label}: the terminal reason must declare tool use ({marker}); body: {rendered}"
    );
}

/// Assertion 3: a truncated turn says it was truncated.
///
/// The mirror of assertion 2. Correcting a generic stop to tool use must not
/// also swallow a real cutoff: the call's arguments are incomplete JSON, so a
/// client told "tool_calls" either fails to parse them or runs the tool with
/// the wrong ones, while "length" tells it to ask for more budget.
pub(super) fn assert_declares_truncation(label: &str, rendered: &str, marker: &str) {
    assert!(
        rendered.contains(marker),
        "{label}: a turn cut off mid-tool-call must report the cutoff ({marker}); body: \
         {rendered}"
    );
}

/// Assertion 4: a truncated turn does not also claim a complete tool call.
///
/// The pair to [`assert_declares_truncation`]. Truncation and tool use are the
/// two mutually exclusive readings of the same frame, and a renderer that
/// emits both leaves the client to pick -- most pick tool use, which is the
/// unparseable-arguments outcome the truncation reason exists to prevent.
pub(super) fn assert_no_complete_tool_use(label: &str, rendered: &str, tool_use_marker: &str) {
    assert!(
        !rendered.contains(tool_use_marker),
        "{label}: a truncated turn must not also declare tool use ({tool_use_marker}); body: \
         {rendered}"
    );
}
