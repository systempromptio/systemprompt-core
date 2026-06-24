use std::collections::HashSet;
use systemprompt_identifiers::{ContextId, DbValue, GatewayConversationId, ToDbValue};

#[test]
fn generate_produces_uuid_format() {
    let id = ContextId::generate();
    assert_eq!(id.as_str().len(), 36);
    uuid::Uuid::parse_str(id.as_str()).expect("generate must produce a valid UUID");
}

#[test]
fn generate_round_trips_through_try_new() {
    let id = ContextId::generate();
    let id2 = ContextId::try_new(id.as_str()).unwrap();
    assert_eq!(id, id2);
}

#[test]
fn generate_unique_across_calls() {
    let ids: HashSet<String> = (0..50)
        .map(|_| ContextId::generate().as_str().to_string())
        .collect();
    assert_eq!(ids.len(), 50);
}

#[test]
fn try_new_accepts_valid_uuid() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    let id = ContextId::try_new(uuid).unwrap();
    assert_eq!(id.as_str(), uuid);
}

#[test]
fn try_new_rejects_empty_string() {
    assert!(ContextId::try_new("").is_err());
}

#[test]
fn try_new_rejects_sentinel_system() {
    assert!(ContextId::try_new("system").is_err());
}

#[test]
fn try_new_rejects_plain_string() {
    assert!(ContextId::try_new("not-a-uuid").is_err());
}

#[test]
fn try_new_rejects_prefixed_id() {
    assert!(ContextId::try_new("ctx_abc123").is_err());
}

#[test]
fn display_format() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    let id = ContextId::try_new(uuid).unwrap();
    assert_eq!(format!("{}", id), uuid);
}

#[test]
fn serde_round_trip() {
    let id = ContextId::generate();
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: ContextId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn serde_rejects_malformed_string() {
    let result: Result<ContextId, _> = serde_json::from_str("\"not-a-uuid\"");
    assert!(result.is_err());
}

#[test]
fn derived_from_gateway_conversation_is_a_valid_uuid() {
    let gw = GatewayConversationId::from_prefix_hash(0xdead_beef_cafe_f00d);
    let ctx = ContextId::derived_from_gateway_conversation(&gw);
    assert_eq!(ctx.as_str().len(), 36);
    uuid::Uuid::parse_str(ctx.as_str()).expect("derivation must yield a parseable UUID");
}

#[test]
fn derived_from_gateway_conversation_is_deterministic() {
    let gw = GatewayConversationId::from_prefix_hash(0x1234_5678_9abc_def0);
    let a = ContextId::derived_from_gateway_conversation(&gw);
    let b = ContextId::derived_from_gateway_conversation(&gw);
    assert_eq!(a, b);
}

#[test]
fn derived_from_gateway_conversation_diverges_on_input() {
    let a =
        ContextId::derived_from_gateway_conversation(&GatewayConversationId::from_prefix_hash(0));
    let b =
        ContextId::derived_from_gateway_conversation(&GatewayConversationId::from_prefix_hash(1));
    assert_ne!(a, b);
}

#[test]
fn derived_from_messaging_is_a_valid_uuid() {
    let ctx = ContextId::derived_from_messaging("slack", "T123", "C456");
    assert_eq!(ctx.as_str().len(), 36);
    uuid::Uuid::parse_str(ctx.as_str()).expect("derivation must yield a parseable UUID");
}

#[test]
fn derived_from_messaging_is_deterministic() {
    let a = ContextId::derived_from_messaging("teams", "tenant-1", "conv-1");
    let b = ContextId::derived_from_messaging("teams", "tenant-1", "conv-1");
    assert_eq!(a, b);
}

#[test]
fn derived_from_messaging_diverges_on_platform_org_and_channel() {
    let base = ContextId::derived_from_messaging("slack", "org", "chan");
    assert_ne!(base, ContextId::derived_from_messaging("teams", "org", "chan"));
    assert_ne!(base, ContextId::derived_from_messaging("slack", "org2", "chan"));
    assert_ne!(base, ContextId::derived_from_messaging("slack", "org", "chan2"));
}

#[test]
fn to_db_value_owned_and_ref() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    let id = ContextId::try_new(uuid).unwrap();
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == uuid));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == uuid));
}
