//! Tests for the OpenAI Responses inbound renderer (response + SSE events).

use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::CanonicalContent;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
use systemprompt_api::services::gateway::protocol::inbound::InboundAdapter;
use systemprompt_api::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;

fn sample_response() -> CanonicalResponse {
    CanonicalResponse {
        id: "resp_1".into(),
        model: "gpt-x".into(),
        content: vec![
            CanonicalContent::Text("answer".into()),
            CanonicalContent::ToolUse {
                id: "t1".into(),
                name: "fn".into(),
                input: json!({"a": 1}),
                signature: None,
            },
            CanonicalContent::Thinking {
                text: "thinking".into(),
                signature: None,
            },
            CanonicalContent::ToolResult {
                tool_use_id: "t1".into(),
                content: vec![],
                is_error: false,
                structured_content: None,
                meta: None,
            },
        ],
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: CanonicalUsage {
            input_tokens: 3,
            output_tokens: 7,
            ..CanonicalUsage::default()
        },
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
    }
}

#[test]
fn render_response_emits_messages_and_function_calls() {
    let inbound = OpenAiResponsesInbound;
    let bytes = inbound.render_response(&sample_response());
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(v["id"], "resp_1");
    assert_eq!(v["model"], "gpt-x");
    assert_eq!(v["status"], "completed");
    assert_eq!(v["usage"]["total_tokens"], 10);
    let output = v["output"].as_array().expect("array");
    assert!(output.iter().any(|o| o["type"] == "message"));
    assert!(output.iter().any(|o| o["type"] == "function_call"));
    assert!(output.iter().any(|o| o["type"] == "reasoning"));
}

#[test]
fn render_response_omits_message_when_no_text() {
    let inbound = OpenAiResponsesInbound;
    let resp = CanonicalResponse {
        id: "r".into(),
        model: "m".into(),
        content: vec![CanonicalContent::ToolUse {
            id: "t".into(),
            name: "f".into(),
            input: json!({}),
            signature: None,
        }],
        stop_reason: None,
        usage: CanonicalUsage::default(),
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
    };
    let bytes = inbound.render_response(&resp);
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let output = v["output"].as_array().unwrap();
    assert!(!output.iter().any(|o| o["type"] == "message"));
}

fn parse_sse_data(bytes: &bytes::Bytes) -> Vec<Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|j| serde_json::from_str(j).unwrap_or(Value::Null))
        .collect()
}

#[test]
fn render_event_message_start_uses_fallback_model_when_empty() {
    let inbound = OpenAiResponsesInbound;
    let ev = CanonicalEvent::MessageStart {
        id: "m1".into(),
        model: String::new(),
        usage: CanonicalUsage::default(),
    };
    let frame = inbound.render_event(&ev, "fallback").expect("frame");
    let v = parse_sse_data(&frame);
    assert_eq!(v[0]["response"]["model"], "fallback");
}

#[test]
fn render_event_covers_all_variants() {
    let inbound = OpenAiResponsesInbound;
    let events = vec![
        CanonicalEvent::ContentBlockStart {
            index: 0,
            block: ContentBlockKind::Text,
        },
        CanonicalEvent::ContentBlockStart {
            index: 1,
            block: ContentBlockKind::ToolUse {
                id: "t".into(),
                name: "f".into(),
                signature: None,
            },
        },
        CanonicalEvent::ContentBlockStart {
            index: 2,
            block: ContentBlockKind::Thinking { signature: None },
        },
        CanonicalEvent::TextDelta {
            index: 0,
            text: "abc".into(),
        },
        CanonicalEvent::ThinkingDelta {
            index: 0,
            text: "...".into(),
        },
        CanonicalEvent::ToolUseDelta {
            index: 1,
            partial_json: "{}".into(),
        },
        CanonicalEvent::ContentBlockStop { index: 0 },
        CanonicalEvent::MessageStop {
            id: "id".into(),
            stop_reason: Some(CanonicalStopReason::MaxTokens),
        },
        CanonicalEvent::MessageStop {
            id: "id".into(),
            stop_reason: None,
        },
        CanonicalEvent::Error("oops \"x\\y\"".into()),
    ];
    for ev in events {
        assert!(inbound.render_event(&ev, "m").is_some(), "{ev:?}");
    }
}

#[test]
fn render_event_usage_delta_is_skipped() {
    let inbound = OpenAiResponsesInbound;
    assert!(
        inbound
            .render_event(&CanonicalEvent::UsageDelta(CanonicalUsage::default()), "m")
            .is_none()
    );
}
