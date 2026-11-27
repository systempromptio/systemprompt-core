use sp_protocol::HOST_ID;
use sp_protocol::names::{native_session_id, session_uuid, session_uuid_string};
use sp_protocol::wire::hooks::{
    DEFAULT_REFUSAL, GovernAnswer, HookContext, HookEvent, ToolResponse, skill_ref,
    stamp_session_metadata,
};

fn context() -> HookContext {
    HookContext {
        session_id: native_session_id("ses_1"),
        native_session_id: "ses_1".to_owned(),
        cwd: "C:\\work".to_owned(),
        native_host: HOST_ID.to_owned(),
    }
}

#[test]
fn lifecycle_events_serialize_flat_with_the_event_name() {
    let event = HookEvent::SessionStart {
        context: context(),
        event_id: "evt-1".to_owned(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["hook_event_name"], "SessionStart");
    assert_eq!(json["native_host"], "harness");
    assert_eq!(json["session_id"], session_uuid_string("ses_1"));
    assert_eq!(json["event_id"], "evt-1");
    assert_eq!(event.ingestion_id(), "evt-1");
    assert_eq!(serde_json::from_value::<HookEvent>(json).unwrap(), event);
}

#[test]
fn tool_events_carry_input_response_and_skill_ref() {
    let event = HookEvent::PostToolUse {
        context: context(),
        tool_name: "skill".to_owned(),
        tool_input: serde_json::json!({ "name": "alpha-notes" }),
        tool_use_id: "call_1".to_owned(),
        tool_response: ToolResponse {
            title: None,
            output: "done".to_owned(),
            metadata: None,
        },
        skill_ref: Some(skill_ref(Some("alpha-tools"), HOST_ID, "alpha-notes")),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["hook_event_name"], "PostToolUse");
    assert_eq!(json["skill_ref"], "alpha-tools:alpha-notes");
    assert_eq!(
        json["tool_response"],
        serde_json::json!({ "output": "done" })
    );
    assert_eq!(event.ingestion_id(), "call_1");
    assert_eq!(skill_ref(None, HOST_ID, "x"), "harness:x");
}

#[test]
fn prompt_and_pre_tool_events() {
    let prompt = HookEvent::UserPromptSubmit {
        context: context(),
        prompt: "hi".to_owned(),
        prompt_id: "msg_1".to_owned(),
    };
    assert_eq!(prompt.ingestion_id(), "msg_1");
    assert_eq!(prompt.name(), "UserPromptSubmit");
    let before = HookEvent::PreToolUse {
        context: context(),
        tool_name: "bash".to_owned(),
        tool_input: serde_json::json!({ "command": "ls" }),
        tool_use_id: "call_2".to_owned(),
    };
    let json = serde_json::to_value(&before).unwrap();
    assert_eq!(json["tool_input"]["command"], "ls");
    assert!(json.get("tool_response").is_none());
}

#[test]
fn govern_answers() {
    let deny: GovernAnswer = serde_json::from_str(
        r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"No shell here"}}"#,
    )
    .unwrap();
    assert_eq!(deny.refusal(), Some("No shell here"));
    let bare: GovernAnswer = serde_json::from_str(
        r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":null}}"#,
    )
    .unwrap();
    assert_eq!(bare.refusal(), Some(DEFAULT_REFUSAL));
    let allow: GovernAnswer =
        serde_json::from_str(r#"{"hookSpecificOutput":{"permissionDecision":"allow"}}"#).unwrap();
    assert_eq!(allow.refusal(), None);
    assert!(serde_json::from_str::<GovernAnswer>("{}").is_err());
}

#[test]
fn session_metadata_is_stamped_once() {
    let session = session_uuid("ses_1");
    let mut body = serde_json::json!({ "model": "m", "messages": [] });
    assert!(stamp_session_metadata(&mut body, &session));
    let stamped = body["metadata"]["user_id"].as_str().unwrap().to_owned();
    assert_eq!(
        stamped,
        format!("{{\"session_id\":\"{}\"}}", session_uuid_string("ses_1"))
    );
    assert!(!stamp_session_metadata(&mut body, &session));
    let mut foreign = serde_json::json!({ "metadata": { "user_id": "someone" } });
    assert!(!stamp_session_metadata(&mut foreign, &session));
    let mut not_object = serde_json::json!([1]);
    assert!(!stamp_session_metadata(&mut not_object, &session));
    let mut odd = serde_json::json!({ "metadata": 5 });
    assert!(!stamp_session_metadata(&mut odd, &session));
}
