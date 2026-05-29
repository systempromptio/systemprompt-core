use std::collections::BTreeMap;

use systemprompt_identifiers::ModelId;
use systemprompt_security::authz::AuthzContext;

fn floor() -> BTreeMap<String, serde_json::Value> {
    let mut f = BTreeMap::new();
    f.insert(
        "boeing.clearance".to_owned(),
        serde_json::json!(["Internal", "CUI"]),
    );
    f
}

#[test]
fn round_trips_through_none_context() {
    let ctx = AuthzContext::none().with_marketplace_floor(&floor());

    assert!(ctx.is_none(), "kind is preserved across the builder");
    assert_eq!(ctx.marketplace_floor(), Some(floor()));
}

#[test]
fn absent_floor_reads_back_none() {
    assert!(AuthzContext::none().marketplace_floor().is_none());
}

#[test]
fn preserves_typed_payload_alongside_floor() {
    let model = ModelId::new("claude");
    let ctx = AuthzContext::gateway_invocation(&model).with_marketplace_floor(&floor());

    assert_eq!(ctx.marketplace_floor(), Some(floor()));
    assert_eq!(
        ctx.gateway_invocation_model().map(|m| m.as_str().to_owned()),
        Some("claude".to_owned()),
        "floor injection leaves the typed model payload intact",
    );
}
