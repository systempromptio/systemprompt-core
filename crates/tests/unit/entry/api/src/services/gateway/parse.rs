//! Unit tests for `services::gateway::parse` — usage and tool-call extraction
//! from a canonical response.

use serde_json::json;
use systemprompt_api::services::gateway::parse::{
    extract_assistant_text, extract_from_canonical,
};
use systemprompt_api::services::gateway::protocol::canonical::CanonicalContent;
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};

fn empty_response() -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "m".into(),
        content: vec![],
        stop_reason: Some(CanonicalStopReason::EndTurn),
        usage: CanonicalUsage::default(),
    }
}

#[test]
fn extract_from_canonical_returns_usage_and_no_tools_for_empty() {
    let mut r = empty_response();
    r.usage = CanonicalUsage {
        input_tokens: 11,
        output_tokens: 22,
    };
    let (usage, tools) = extract_from_canonical(&r);
    assert_eq!(usage.input_tokens, 11);
    assert_eq!(usage.output_tokens, 22);
    assert!(tools.is_empty());
}

#[test]
fn extract_from_canonical_captures_tool_uses() {
    let mut r = empty_response();
    r.content = vec![
        CanonicalContent::Text("hi".into()),
        CanonicalContent::ToolUse {
            id: "tu_1".into(),
            name: "search".into(),
            input: json!({"query": "rust"}),
        },
        CanonicalContent::ToolUse {
            id: "tu_2".into(),
            name: "fetch".into(),
            input: json!({"url": "https://x"}),
        },
    ];
    let (_, tools) = extract_from_canonical(&r);
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].ai_tool_call_id, "tu_1");
    assert_eq!(tools[0].tool_name, "search");
    assert!(tools[0].tool_input.contains("rust"));
    assert_eq!(tools[1].tool_name, "fetch");
}

#[test]
fn extract_assistant_text_returns_none_when_no_text() {
    let r = empty_response();
    assert!(extract_assistant_text(&r).is_none());
}

#[test]
fn extract_assistant_text_joins_text_parts_with_newline() {
    let mut r = empty_response();
    r.content = vec![
        CanonicalContent::Text("hello".into()),
        CanonicalContent::ToolUse {
            id: "x".into(),
            name: "y".into(),
            input: json!({}),
        },
        CanonicalContent::Text("world".into()),
    ];
    assert_eq!(extract_assistant_text(&r).as_deref(), Some("hello\nworld"));
}

#[test]
fn extract_assistant_text_single_text_no_newline() {
    let mut r = empty_response();
    r.content = vec![CanonicalContent::Text("solo".into())];
    assert_eq!(extract_assistant_text(&r).as_deref(), Some("solo"));
}

#[test]
fn extract_assistant_text_skips_non_text_variants() {
    let mut r = empty_response();
    r.content = vec![
        CanonicalContent::Thinking {
            text: "thought".into(),
            signature: None,
        },
        CanonicalContent::ToolUse {
            id: "t".into(),
            name: "n".into(),
            input: json!({}),
        },
    ];
    assert!(extract_assistant_text(&r).is_none());
}
