use serde_json::{Value, json};
use systemprompt_bridge::gui::ipc::{
    BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, emit_script, reply_script,
};

#[test]
fn new_sets_all_fields_with_no_detail() {
    let err = BridgeError::new(ErrorScope::Gateway, ErrorCode::Unreachable, "boom");
    assert_eq!(err.message, "boom");
    assert!(err.detail.is_none());

    let value = serde_json::to_value(&err).expect("serialize");
    assert_eq!(value["scope"], json!("gateway"));
    assert_eq!(value["code"], json!("unreachable"));
    assert_eq!(value["message"], json!("boom"));
    assert!(value.get("detail").is_none(), "detail must be skipped when None");
}

#[test]
fn with_detail_attaches_and_serializes_detail() {
    let err = BridgeError::new(ErrorScope::Host, ErrorCode::Conflict, "clash")
        .with_detail(json!({ "host": "claude" }));
    assert_eq!(err.detail, Some(json!({ "host": "claude" })));

    let value = serde_json::to_value(&err).expect("serialize");
    assert_eq!(value["detail"], json!({ "host": "claude" }));
}

#[test]
fn invalid_args_constructor() {
    let err = BridgeError::invalid_args("bad");
    let value = serde_json::to_value(&err).expect("serialize");
    assert_eq!(value["scope"], json!("internal"));
    assert_eq!(value["code"], json!("invalid_args"));
    assert_eq!(value["message"], json!("bad"));
}

#[test]
fn not_found_constructor() {
    let err = BridgeError::not_found("missing");
    let value = serde_json::to_value(&err).expect("serialize");
    assert_eq!(value["scope"], json!("internal"));
    assert_eq!(value["code"], json!("not_found"));
    assert_eq!(value["message"], json!("missing"));
}

#[test]
fn internal_constructor() {
    let err = BridgeError::internal("oops");
    let value = serde_json::to_value(&err).expect("serialize");
    assert_eq!(value["scope"], json!("internal"));
    assert_eq!(value["code"], json!("internal"));
    assert_eq!(value["message"], json!("oops"));
}

#[test]
fn reply_script_contains_id_and_payload_json() {
    let payload = IpcReplyPayload::ok(json!({ "answer": 42 }));
    let script = reply_script(7, &payload);

    let body = serde_json::to_string(&payload).expect("serialize payload");
    assert!(script.contains("reply(7, "), "script should embed the request id: {script}");
    assert!(script.contains(&body), "script should embed the payload JSON: {script}");
}

#[test]
fn reply_script_err_payload_carries_error() {
    let payload = IpcReplyPayload::err(BridgeError::not_found("nope"));
    let script = reply_script(11, &payload);

    assert!(script.contains("reply(11, "));
    assert!(script.contains("\"ok\":false"));
    assert!(script.contains("not_found"));
}

#[test]
fn emit_script_contains_channel_and_payload() {
    let payload: Value = json!({ "phase": "done" });
    let script = emit_script("sync.progress", &payload);

    assert!(
        script.contains("\"sync.progress\""),
        "script should embed the JSON-encoded channel: {script}"
    );
    let body = serde_json::to_string(&payload).expect("serialize payload");
    assert!(script.contains(&body), "script should embed the payload JSON: {script}");
}
