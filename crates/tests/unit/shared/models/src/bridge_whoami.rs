use std::collections::BTreeMap;

use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::gateway::WhoamiResponse;

fn attributes() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        (
            "group".to_owned(),
            vec!["delivery".to_owned(), "leads".to_owned()],
        ),
        ("project".to_owned(), Vec::new()),
    ])
}

#[test]
fn whoami_carries_the_attributes_per_dimension() {
    let body = serde_json::to_value(WhoamiResponse {
        user_id: Some(UserId::new("user-1")),
        roles: vec!["admin".to_owned()],
        attributes: attributes(),
        ..WhoamiResponse::default()
    })
    .unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "user_id": "user-1",
            "roles": ["admin"],
            "attributes": { "group": ["delivery", "leads"], "project": [] },
        })
    );
}

#[test]
fn whoami_omits_attributes_when_no_dimension_is_declared() {
    let body = serde_json::to_value(WhoamiResponse::default()).unwrap();
    assert_eq!(body, serde_json::json!({ "roles": [] }));
}

#[test]
fn whoami_reads_attributes_as_its_own_field_and_not_as_an_extra() {
    let decoded: WhoamiResponse = serde_json::from_value(serde_json::json!({
        "user_id": "user-1",
        "attributes": { "group": ["delivery", "leads"], "project": [] },
        "department": "Delivery",
    }))
    .unwrap();
    assert_eq!(decoded.attributes, attributes());
    assert_eq!(decoded.extra.keys().collect::<Vec<_>>(), vec!["department"]);
}

#[test]
fn whoami_from_a_gateway_without_attributes_decodes_them_empty() {
    let decoded: WhoamiResponse =
        serde_json::from_value(serde_json::json!({ "user_id": "user-1", "roles": ["user"] }))
            .unwrap();
    assert!(decoded.attributes.is_empty());
    assert!(decoded.extra.is_empty());
}
