//! `RunEventPayload` round-trips through serde with its `kind` tag.

use systemprompt_headless::{HookDecision, RunEventKind, RunEventPayload};
use systemprompt_identifiers::HostSessionId;

fn round_trip(payload: &RunEventPayload) -> RunEventPayload {
    let json = serde_json::to_string(payload).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

#[test]
fn every_kind_round_trips() {
    let payloads = vec![
        RunEventPayload::Init {
            session_id: HostSessionId::try_new("sess-1").expect("host session id"),
            model: "claude-sonnet-5-5".into(),
            tools: vec!["Bash".into()],
            cwd: Some("/work".into()),
        },
        RunEventPayload::AssistantText { text: "hi".into() },
        RunEventPayload::ToolUse {
            tool_use_id: "tu_1".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        },
        RunEventPayload::Hook {
            name: "PreToolUse".into(),
            decision: HookDecision::Deny,
            policy: Some("blocklist".into()),
        },
        RunEventPayload::Result {
            subtype: "success".into(),
            is_error: false,
            num_turns: 3,
            duration_ms: 1200,
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 4500,
            structured_output: None,
            error: None,
        },
    ];
    for payload in &payloads {
        assert_eq!(&round_trip(payload), payload);
    }
}

#[test]
fn kind_tag_matches_column_spelling() {
    let payload = RunEventPayload::AssistantText { text: "x".into() };
    let value = serde_json::to_value(&payload).expect("serialize");
    assert_eq!(value["kind"], "assistant_text");
    assert_eq!(payload.kind(), RunEventKind::AssistantText);
    assert_eq!(payload.kind().as_str(), "assistant_text");
}
