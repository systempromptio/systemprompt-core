//! A field edit merges onto the stored fields: a value replaces, `null`
//! removes, an omitted key is kept, and the merged map is checked against
//! the kind by the schema validator's field check.

use serde_json::{Map, Value, json};
use systemprompt_brain::defaults::SchemaValidator;
use systemprompt_brain::models::merge_fields;

use crate::pipeline_fixtures::kind_decl;

fn map(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("object")
}

#[test]
fn a_value_replaces_null_removes_and_an_omitted_key_is_kept() {
    let stored = map(json!({"price": "1200", "region": "emea", "note": "list"}));
    let merged = merge_fields(&stored, map(json!({"price": "1500", "note": null})));
    assert_eq!(merged, map(json!({"price": "1500", "region": "emea"})));
}

#[test]
fn an_empty_edit_keeps_the_stored_fields() {
    let stored = map(json!({"price": "1200"}));
    assert_eq!(merge_fields(&stored, Map::new()), stored);
}

#[test]
fn the_field_check_accepts_a_valid_map_and_refuses_what_the_kind_does_not_allow() {
    let kind = kind_decl();
    let valid = map(json!({"owner": "billing", "status": "open", "amount": 12}));
    assert_eq!(SchemaValidator::fields_reason(&valid, &kind), None);

    let removed = merge_fields(&valid, map(json!({"status": null})));
    assert_eq!(
        SchemaValidator::fields_reason(&removed, &kind),
        None,
        "an optional field may be removed"
    );
    for bad in [
        merge_fields(&valid, map(json!({"owner": null}))),
        merge_fields(&valid, map(json!({"status": "pending"}))),
        merge_fields(&valid, map(json!({"amount": "twelve"}))),
        merge_fields(&valid, map(json!({"never_declared": "x"}))),
    ] {
        assert!(
            SchemaValidator::fields_reason(&bad, &kind).is_some(),
            "{bad:?}"
        );
    }
}
