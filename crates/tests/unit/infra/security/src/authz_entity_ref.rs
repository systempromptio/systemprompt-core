use systemprompt_security::authz::{EntityKind, EntityRef};

#[test]
fn from_kind_and_id_round_trips_every_kind() {
    for kind in EntityKind::ALL {
        let entity = EntityRef::from_kind_and_id(*kind, "some-id").expect("non-empty id");
        assert_eq!(entity.kind(), *kind);
        assert_eq!(entity.id_str(), "some-id");
    }
}

#[test]
fn from_kind_and_id_rejects_an_empty_mcp_server_id() {
    let err = EntityRef::from_kind_and_id(EntityKind::McpServer, "")
        .expect_err("McpServerId is non-empty");
    assert!(err.to_string().contains("McpServerId"));
}

#[test]
fn all_kinds_are_distinct() {
    let mut kinds: Vec<&str> = EntityKind::ALL.iter().map(|k| k.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds.len(), EntityKind::ALL.len());
}
