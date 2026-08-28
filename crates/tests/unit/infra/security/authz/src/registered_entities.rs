//! `RegisteredEntities`: which literal ids ingestion and the admin write paths
//! accept, per kind.

use systemprompt_security::authz::{EntityKind, RegisteredEntities};

#[test]
fn an_unenforced_kind_permits_any_id() {
    let registered = RegisteredEntities::default();
    assert!(registered.permits(EntityKind::GatewayRoute, "anything-at-all"));
    registered
        .require(EntityKind::GatewayRoute, "anything-at-all")
        .expect("unenforced kinds are not checked");
}

#[test]
fn an_enforced_kind_permits_only_registered_ids() {
    let registered =
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, ["claude-star-4203d1"]);
    assert!(registered.permits(EntityKind::GatewayRoute, "claude-star-4203d1"));
    assert!(!registered.permits(EntityKind::GatewayRoute, "claude-star-000000"));
    assert!(
        registered.permits(EntityKind::McpServer, "odoo"),
        "enforcement is per kind; a kind that was never declared stays open"
    );
}

#[test]
fn an_enforced_empty_set_rejects_every_id() {
    let registered =
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, Vec::<String>::new());
    assert!(!registered.permits(EntityKind::GatewayRoute, "claude-star-4203d1"));
    let err = registered
        .require(EntityKind::GatewayRoute, "claude-star-4203d1")
        .expect_err("an empty declared set means none exist");
    assert!(
        err.to_string().contains("no gateway_route is registered"),
        "message must say the catalog is empty: {err}"
    );
}

#[test]
fn known_ids_are_sorted_and_empty_for_undeclared_kinds() {
    let registered =
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, ["zeta", "alpha", "mid"]);
    assert_eq!(
        registered.known_ids(EntityKind::GatewayRoute),
        vec!["alpha", "mid", "zeta"]
    );
    assert!(registered.known_ids(EntityKind::McpServer).is_empty());
}

#[test]
fn rejection_names_the_id_the_catalog_and_for_routes_the_fix() {
    let registered =
        RegisteredEntities::new().with_kind(EntityKind::GatewayRoute, ["claude-star-4203d1"]);
    let err = registered
        .require(EntityKind::GatewayRoute, "claude-opus-4-8-gemini")
        .expect_err("unregistered");
    let msg = err.to_string();
    assert!(msg.contains("claude-opus-4-8-gemini"), "{msg}");
    assert!(msg.contains("claude-star-4203d1"), "{msg}");
    assert!(msg.contains("entity_match"), "{msg}");

    let registered = RegisteredEntities::new().with_kind(EntityKind::McpServer, ["odoo"]);
    let msg = registered
        .require(EntityKind::McpServer, "salesforce")
        .expect_err("unregistered")
        .to_string();
    assert!(msg.contains("salesforce") && msg.contains("odoo"), "{msg}");
    assert!(
        !msg.contains("entity_match"),
        "the generated-id hint is for routes only: {msg}"
    );
}
