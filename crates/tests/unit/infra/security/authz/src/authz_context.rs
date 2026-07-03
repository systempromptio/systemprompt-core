use std::collections::BTreeMap;

use serde_json::json;
use systemprompt_identifiers::{McpToolName, ModelId, RouteId, TraceId, UserId};
use systemprompt_security::authz::{AuthzContext, AuthzDecision, AuthzRequest, EntityRef};

fn make_request(ctx: AuthzContext) -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("r1")),
        user_id: UserId::new("u1"),
        roles: vec![],
        attributes: BTreeMap::new(),
        trace_id: TraceId::new("t1"),
        session_id: None,
        context: ctx,
        context_id: None,
        task_id: None,
        act_chain: vec![],
    }
}

#[test]
fn none_context_is_none() {
    let ctx = AuthzContext::none();
    assert!(ctx.is_none());
    assert_eq!(ctx.kind.as_ref(), "none");
}

#[test]
fn default_context_is_none() {
    let ctx = AuthzContext::default();
    assert!(ctx.is_none());
}

#[test]
fn gateway_invocation_context() {
    let model = ModelId::new("claude-3-opus");
    let ctx = AuthzContext::gateway_invocation(&model);
    assert!(!ctx.is_none());
    assert_eq!(ctx.kind.as_ref(), "gateway.invocation");
    let extracted = ctx.gateway_invocation_model().expect("model");
    assert_eq!(extracted.as_str(), "claude-3-opus");
}

#[test]
fn gateway_invocation_model_wrong_kind_returns_none() {
    let ctx = AuthzContext::none();
    assert!(ctx.gateway_invocation_model().is_none());
}

#[test]
fn mcp_tool_call_context() {
    let tool = McpToolName::new("bash");
    let ctx = AuthzContext::mcp_tool_call(&tool);
    assert_eq!(ctx.kind.as_ref(), "mcp.tool_call");
    let extracted = ctx.mcp_tool_call_tool().expect("tool");
    assert_eq!(extracted.as_str(), "bash");
}

#[test]
fn mcp_tool_call_wrong_kind_returns_none() {
    let ctx = AuthzContext::none();
    assert!(ctx.mcp_tool_call_tool().is_none());
}

#[test]
fn extension_context_stores_kind_and_payload() {
    let payload = json!({ "clearance": "secret" });
    let ctx = AuthzContext::extension("acme.order_submission", payload.clone());
    assert_eq!(ctx.kind.as_ref(), "acme.order_submission");
    assert!(!ctx.is_none());
}

#[test]
fn authz_context_serde_none() {
    let ctx = AuthzContext::none();
    let s = serde_json::to_string(&ctx).unwrap();
    assert!(s.contains("\"kind\":\"none\""), "got: {s}");
    let back: AuthzContext = serde_json::from_str(&s).unwrap();
    assert!(back.is_none());
}

#[test]
fn marketplace_floor_roundtrip() {
    let model = ModelId::new("claude-3-5-sonnet");
    let base_ctx = AuthzContext::gateway_invocation(&model);

    let mut floor: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    floor.insert("tier".to_owned(), json!("pro"));
    floor.insert("region".to_owned(), json!("us-east-1"));

    let ctx_with_floor = base_ctx.with_marketplace_floor(&floor);
    assert_eq!(ctx_with_floor.kind.as_ref(), "gateway.invocation");

    let retrieved = ctx_with_floor.marketplace_floor().expect("floor");
    assert_eq!(retrieved.get("tier").unwrap(), &json!("pro"));
    assert_eq!(retrieved.get("region").unwrap(), &json!("us-east-1"));
}

#[test]
fn marketplace_floor_on_none_context() {
    let ctx = AuthzContext::none();
    assert!(ctx.marketplace_floor().is_none());
}

#[test]
fn marketplace_floor_not_present_returns_none() {
    let model = ModelId::new("claude-3-haiku");
    let ctx = AuthzContext::gateway_invocation(&model);
    assert!(ctx.marketplace_floor().is_none());
}

#[test]
fn authz_decision_allow_serde() {
    let d = AuthzDecision::Allow;
    let s = serde_json::to_string(&d).unwrap();
    assert!(s.contains("\"decision\":\"allow\""), "got: {s}");
    let back: AuthzDecision = serde_json::from_str(&s).unwrap();
    assert_eq!(back, AuthzDecision::Allow);
}

#[test]
fn authz_request_serde_roundtrip() {
    let ctx = AuthzContext::none();
    let req = make_request(ctx);
    let s = serde_json::to_string(&req).unwrap();
    let back: AuthzRequest = serde_json::from_str(&s).unwrap();
    assert_eq!(back.user_id.as_str(), "u1");
    assert!(back.context.is_none());
}
