use systemprompt_security::authz::EntityKind;
use systemprompt_security::authz::types::EntityRow;

#[test]
fn entity_row_round_trips_through_serde() {
    let row = EntityRow {
        kind: EntityKind::GatewayRoute,
        id: "claude-star".to_owned(),
        default_included: true,
        source: "profile:local".to_owned(),
    };
    let serialised = serde_json::to_string(&row).unwrap();
    assert!(
        serialised.contains("\"kind\":\"gateway_route\""),
        "got: {serialised}"
    );
    let parsed: EntityRow = serde_json::from_str(&serialised).unwrap();
    assert_eq!(parsed, row);
}

#[test]
fn access_rule_no_longer_carries_default_included() {
    // Regression guard for migration 007 — `default_included` moved to
    // EntityRow. If this test starts compiling after a future edit re-adds
    // the field on AccessRule the storage split has been silently undone.
    let json = r#"{"id":"r1","rule_type":"role","rule_value":"user","access":"allow"}"#;
    let parsed: systemprompt_security::authz::AccessRule = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.rule_value, "user");
}
