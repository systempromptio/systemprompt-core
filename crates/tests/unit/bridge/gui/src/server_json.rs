use serde_json::json;
use systemprompt_bridge::gui::server_json::{identity_value, mcp_auth_value};
use systemprompt_bridge::gui::state::{AppStateSnapshot, VerifiedIdentity};
use systemprompt_identifiers::{TenantId, UserId};

#[test]
fn identity_value_is_null_without_identity() {
    let snap = AppStateSnapshot::default();
    assert_eq!(identity_value(&snap), serde_json::Value::Null);
}

#[test]
fn identity_value_serializes_present_identity() {
    let mut snap = AppStateSnapshot::default();
    snap.verified_identity = Some(VerifiedIdentity {
        email: Some("a@b.com".to_owned()),
        user_id: Some(UserId::new("user_1")),
        tenant_id: Some(TenantId::new("tenant_1")),
        exp_unix: Some(1_893_456_000),
        verified_at_unix: 1_700_000_000,
    });

    let value = identity_value(&snap);
    assert_eq!(value["email"], json!("a@b.com"));
    assert_eq!(value["user_id"], json!("user_1"));
    assert_eq!(value["tenant_id"], json!("tenant_1"));
    assert_eq!(value["exp_unix"], json!(1_893_456_000_u64));
    assert_eq!(value["verified_at_unix"], json!(1_700_000_000_u64));
}

#[test]
fn mcp_auth_value_reflects_snapshot_flags() {
    let mut snap = AppStateSnapshot::default();
    snap.mcp_auth_probe_in_flight = true;

    let value = mcp_auth_value(&snap);
    assert_eq!(value["probing"], json!(true));
    assert_eq!(value["servers"], json!([]));
}
