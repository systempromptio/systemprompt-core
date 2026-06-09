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
        CanonicalEvent::Error("oops \"x\\y\"".into()),
    ];
    for ev in events {
        assert!(inbound.render_event(&ev, "m").is_some(), "{ev:?}");
    }
}

#[test]
fn render_event_skips_usage_and_terminal_events() {
    let inbound = OpenAiResponsesInbound;
    let skipped = vec![
        CanonicalEvent::UsageDelta(CanonicalUsage::default()),
        CanonicalEvent::ContentBlockStop { index: 0 },
        CanonicalEvent::MessageStop {
            id: "id".into(),
            stop_reason: Some(CanonicalStopReason::ToolUse),
        },
    ];
    for ev in skipped {
        assert!(
            inbound.render_event(&ev, "m").is_none(),
            "terminal/usage events render only from the snapshot: {ev:?}"
        );
    }
}

#[test]
fn render_terminal_item_done_carries_finalized_function_call() {
    let inbound = OpenAiResponsesInbound;
    let snapshot = sample_response();
    let frame = inbound
        .render_terminal_event(
            &CanonicalEvent::ContentBlockStop { index: 1 },
            &snapshot,
            "m",
        )
        .expect("terminal frame");
    let frames = parse_sse_data(&frame);
    let args_done = frames
        .iter()
        .find(|f| f["type"] == "response.function_call_arguments.done")
        .expect("function_call_arguments.done frame");
    assert_eq!(args_done["arguments"], "{\"a\":1}");
    let item_done = frames
        .iter()
        .find(|f| f["type"] == "response.output_item.done")
        .expect("output_item.done frame");
    assert_eq!(item_done["item"]["type"], "function_call");
    assert_eq!(item_done["item"]["call_id"], "t1");
    assert_eq!(item_done["item"]["name"], "fn");
    assert_eq!(item_done["item"]["arguments"], "{\"a\":1}");
    assert_eq!(item_done["item"]["status"], "completed");
}

#[test]
fn render_terminal_completed_carries_full_output_list() {
    let inbound = OpenAiResponsesInbound;
    let mut snapshot = sample_response();
    snapshot.stop_reason = Some(CanonicalStopReason::ToolUse);
    let frame = inbound
        .render_terminal_event(
            &CanonicalEvent::MessageStop {
                id: "resp_1".into(),
                stop_reason: Some(CanonicalStopReason::ToolUse),
            },
            &snapshot,
            "m",
        )
        .expect("terminal frame");
    let frames = parse_sse_data(&frame);
    let completed = frames
        .iter()
        .find(|f| f["type"] == "response.completed")
        .expect("response.completed frame");
    assert_eq!(completed["response"]["status"], "completed");
    assert_eq!(completed["response"]["stop_reason"], "tool_calls");
    let output = completed["response"]["output"]
        .as_array()
        .expect("output array");
    assert!(
        output.iter().any(|o| o["type"] == "function_call"
            && o["call_id"] == "t1"
            && o["arguments"] == "{\"a\":1}"),
        "completed.output must carry the finalized function call: {output:?}"
    );
}

#[test]
fn render_terminal_incomplete_maps_to_incomplete_status() {
    let inbound = OpenAiResponsesInbound;
    let mut snapshot = sample_response();
    snapshot.content = vec![CanonicalContent::Text("partial".into())];
    let frame = inbound
        .render_terminal_event(
            &CanonicalEvent::MessageStop {
                id: "resp_1".into(),
                stop_reason: Some(CanonicalStopReason::MaxTokens),
            },
            &snapshot,
            "m",
        )
        .expect("terminal frame");
    let frames = parse_sse_data(&frame);
    let incomplete = frames
        .iter()
        .find(|f| f["type"] == "response.incomplete")
        .expect("response.incomplete frame");
    assert_eq!(incomplete["response"]["status"], "incomplete");
    assert_eq!(
        incomplete["response"]["incomplete_details"]["reason"],
        "max_output_tokens"
    );
}
