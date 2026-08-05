use std::collections::HashSet;
use systemprompt_identifiers::{
    ContextId, DbValue, EvalRunId, GatewayConversationId, SessionId, TaskId, ToDbValue,
};

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
    assert_ne!(
        base,
        ContextId::derived_from_messaging("teams", "org", "chan")
    );
    assert_ne!(
        base,
        ContextId::derived_from_messaging("slack", "org2", "chan")
    );
    assert_ne!(
        base,
        ContextId::derived_from_messaging("slack", "org", "chan2")
    );
}

#[test]
fn to_db_value_owned_and_ref() {
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    let id = ContextId::try_new(uuid).unwrap();
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == uuid));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == uuid));
}

#[test]
fn derived_from_session_is_pinned_to_its_namespace_forever() {
    let ctx = ContextId::derived_from_session(&SessionId::new("sess-1"));
    assert_eq!(
        ctx.as_str(),
        "0cb9c4c8-6b84-5c1a-b6e6-a148690fa761",
        "a different value re-homes every historical session-derived context"
    );
    assert_eq!(
        ctx,
        ContextId::derived_from_session(&SessionId::new("sess-1"))
    );
    assert_ne!(
        ctx,
        ContextId::derived_from_session(&SessionId::new("sess-2"))
    );
}

#[test]
fn derived_from_evaluation_run_is_pinned_and_deterministic() {
    let ctx = ContextId::derived_from_evaluation_run(&EvalRunId::new("run-1"));
    assert_eq!(ctx.as_str(), "6bcd29eb-7ce3-5eeb-a8a4-3f986aab216e");
    assert_ne!(
        ctx,
        ContextId::derived_from_evaluation_run(&EvalRunId::new("run-2"))
    );
}

#[test]
fn derived_from_cli_probe_is_pinned_and_deterministic() {
    let ctx = ContextId::derived_from_cli_probe("server-a");
    assert_eq!(ctx.as_str(), "f85364b9-1f5b-527b-935f-22e274e31de7");
    assert_ne!(ctx, ContextId::derived_from_cli_probe("server-b"));
}

#[test]
fn derived_from_mcp_validation_is_pinned_and_deterministic() {
    let ctx = ContextId::derived_from_mcp_validation("svc-a");
    assert_eq!(ctx.as_str(), "c16920e8-3662-5e00-b45f-373ff2ee14e3");
    assert_ne!(ctx, ContextId::derived_from_mcp_validation("svc-b"));
}

#[test]
fn derived_from_task_is_pinned_and_deterministic() {
    let ctx = ContextId::derived_from_task(&TaskId::new("task-1"));
    assert_eq!(ctx.as_str(), "d305f9fe-290a-5ca5-856a-b1f69f08c495");
    assert_ne!(ctx, ContextId::derived_from_task(&TaskId::new("task-2")));
}

#[test]
fn the_legacy_context_is_a_fixed_parseable_uuid() {
    let a = ContextId::legacy();
    assert_eq!(a.as_str(), "00000000-0000-0000-0000-4c4547414359");
    uuid::Uuid::parse_str(a.as_str()).expect("legacy context must be a valid UUID");
    assert_eq!(a, ContextId::legacy());
}

#[test]
fn every_derivation_namespace_is_disjoint_for_the_same_key() {
    let key = "same-key";
    let ids = [
        ContextId::derived_from_session(&SessionId::new(key)),
        ContextId::derived_from_evaluation_run(&EvalRunId::new(key)),
        ContextId::derived_from_cli_probe(key),
        ContextId::derived_from_mcp_validation(key),
        ContextId::derived_from_task(&TaskId::new(key)),
    ];
    let unique: HashSet<&str> = ids.iter().map(ContextId::as_str).collect();
    assert_eq!(unique.len(), ids.len());
}
