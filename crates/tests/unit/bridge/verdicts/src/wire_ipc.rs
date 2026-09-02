//! The IPC envelope the webview and the bridge exchange: request parsing,
//! reply shape, and the two script snippets that carry them into the page.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_bridge::wire::ipc::{
    BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, IpcRequest, emit_script, reply_script,
};

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("payload serialises")
}

#[test]
fn error_scopes_and_codes_serialise_as_snake_case_the_catalogue_keys_on() {
    assert_eq!(json_of(&ErrorScope::Gateway), json!("gateway"));
    assert_eq!(json_of(&ErrorScope::Marketplace), json!("marketplace"));
    assert_eq!(json_of(&ErrorScope::Internal), json!("internal"));
    assert_eq!(json_of(&ErrorCode::InvalidArgs), json!("invalid_args"));
    assert_eq!(json_of(&ErrorCode::InvalidFormat), json!("invalid_format"));
    assert_eq!(json_of(&ErrorCode::NotFound), json!("not_found"));
    assert_eq!(json_of(&ErrorCode::Unauthorized), json!("unauthorized"));
}

#[test]
fn a_bridge_error_without_detail_omits_the_field() {
    let err = BridgeError::new(ErrorScope::Host, ErrorCode::Timeout, "probe timed out");
    let v = json_of(&err);

    assert_eq!(v["scope"], json!("host"));
    assert_eq!(v["code"], json!("timeout"));
    assert_eq!(v["message"], json!("probe timed out"));
    assert!(v.get("detail").is_none(), "absent detail must vanish: {v}");
}

#[test]
fn with_detail_attaches_the_payload_verbatim() {
    let err = BridgeError::new(ErrorScope::Proxy, ErrorCode::Unreachable, "refused")
        .with_detail(json!({ "port": 8899 }));
    let v = json_of(&err);

    assert_eq!(v["detail"]["port"], json!(8899));
    assert_eq!(v["code"], json!("unreachable"));
}

#[test]
fn the_error_constructors_pick_their_own_code_and_scope() {
    let invalid = json_of(&BridgeError::invalid_args("bad id"));
    assert_eq!(invalid["scope"], json!("internal"));
    assert_eq!(invalid["code"], json!("invalid_args"));
    assert_eq!(invalid["message"], json!("bad id"));

    let missing = json_of(&BridgeError::not_found("no such host"));
    assert_eq!(missing["code"], json!("not_found"));

    let boom = json_of(&BridgeError::internal("unexpected"));
    assert_eq!(boom["code"], json!("internal"));
    assert_eq!(boom["message"], json!("unexpected"));
}

#[test]
fn an_ok_reply_carries_the_value_and_no_error_key() {
    let v = json_of(&IpcReplyPayload::ok(json!({ "count": 3 })));

    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["value"]["count"], json!(3));
    assert!(v.get("error").is_none(), "an ok reply has no error: {v}");
}

#[test]
fn an_error_reply_carries_the_error_and_no_value_key() {
    let v = json_of(&IpcReplyPayload::err(BridgeError::not_found("gone")));

    assert_eq!(v["ok"], json!(false));
    assert_eq!(v["error"]["code"], json!("not_found"));
    assert_eq!(v["error"]["message"], json!("gone"));
    assert!(v.get("value").is_none(), "an error reply has no value: {v}");
}

#[test]
fn an_ipc_request_without_args_deserialises_to_a_null_value() {
    let req: IpcRequest =
        serde_json::from_str(r#"{"id":7,"cmd":"state.refresh"}"#).expect("request parses");

    assert_eq!(req.id, 7);
    assert_eq!(req.cmd, "state.refresh");
    assert_eq!(req.args, Value::Null);
}

#[test]
fn an_ipc_request_keeps_its_args_untouched() {
    let req: IpcRequest =
        serde_json::from_str(r#"{"id":42,"cmd":"host.probe","args":{"host_id":"codex"}}"#)
            .expect("request parses");

    assert_eq!(req.id, 42);
    assert_eq!(req.cmd, "host.probe");
    assert_eq!(req.args["host_id"], json!("codex"));
}

#[test]
fn an_ipc_request_missing_its_id_is_rejected() {
    let parsed = serde_json::from_str::<IpcRequest>(r#"{"cmd":"state.refresh"}"#);

    assert!(parsed.is_err(), "a request with no id must not parse");
}

#[test]
fn reply_script_guards_the_bridge_handle_and_embeds_the_id_and_body() {
    let script = reply_script(9, &IpcReplyPayload::ok(json!({ "a": 1 })));

    assert!(
        script.starts_with("window.__bridge && window.__bridge.reply && window.__bridge.reply(9, "),
        "unexpected script: {script}"
    );
    assert!(
        script.contains(r#"{"ok":true,"value":{"a":1}}"#),
        "the reply body must be inlined: {script}"
    );
    assert!(script.ends_with(");"), "unexpected script tail: {script}");
}

#[test]
fn emit_script_json_encodes_the_channel_name_rather_than_pasting_it() {
    let script = emit_script("state.changed", &json!({ "signed_in": true }));

    assert!(
        script.contains(r#"window.__bridge.emit("state.changed", {"signed_in":true})"#),
        "unexpected script: {script}"
    );
}

#[test]
fn a_channel_name_with_quotes_cannot_break_out_of_the_emitted_call() {
    let script = emit_script(r#"evil");alert(1);//"#, &Value::Null);

    assert!(
        !script.contains(r#"emit("evil");alert(1)"#),
        "the channel name escaped its string literal: {script}"
    );
    assert!(
        script.contains(r#"\"};alert(1);//"#) || script.contains(r#"evil\");alert(1);//"#),
        "the channel name must be json-escaped: {script}"
    );
}
