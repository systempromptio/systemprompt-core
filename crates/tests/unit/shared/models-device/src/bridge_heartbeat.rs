use systemprompt_identifiers::SessionId;
use systemprompt_models::bridge::heartbeat::{
    ClientState, HeartbeatRequest, HeartbeatResponse, saturating_total,
};
use systemprompt_models::bridge::instance::WindowStatus;

#[test]
fn the_request_carries_null_before_the_first_call() {
    let body = HeartbeatRequest {
        session_id: SessionId::new("sess_1"),
        bridge_version: "0.87.0".to_owned(),
        os: "windows".to_owned(),
        hostname: "WS-01".to_owned(),
        last_activity_at: None,
        forwarded_total: 3,
        tokens_in_total: 100,
        tokens_out_total: 50,
        release_version: "1.0.0".to_owned(),
        state: ClientState::Active,
    };
    let json = serde_json::to_value(&body).unwrap();
    assert!(json["last_activity_at"].is_null());
    assert_eq!(json["state"], "active");
    assert_eq!(json["session_id"], "sess_1");
    assert_eq!(saturating_total(u64::MAX), i64::MAX);
}

#[test]
fn answers_decode_and_tell_when_unsupported() {
    let ok: HeartbeatResponse = serde_json::from_str(
        r#"{"min_bridge_version":"0.28.0","compatible":true,"window_status":"within"}"#,
    )
    .unwrap();
    assert!(!ok.unsupported());
    let above: HeartbeatResponse = serde_json::from_str(
        r#"{"min_bridge_version":"0.28.0","compatible":true,"window_status":"above"}"#,
    )
    .unwrap();
    assert!(above.unsupported());
    let old: HeartbeatResponse =
        serde_json::from_str(r#"{"min_bridge_version":"9.0.0","compatible":false}"#).unwrap();
    assert!(old.unsupported());
    let future: HeartbeatResponse = serde_json::from_str(
        r#"{"min_bridge_version":"0.28.0","compatible":true,"window_status":"sideways"}"#,
    )
    .unwrap();
    assert_eq!(future.window_status, Some(WindowStatus::Unknown));
}
