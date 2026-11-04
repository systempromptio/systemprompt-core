//! Alias derivation for a stored fact's subject is deterministic: the id,
//! the id with separators as spaces, the display name and every
//! `alias: true` field, each once, in that order.

use std::collections::BTreeMap;

use systemprompt_brain::index::{AliasInput, derive_aliases};
use systemprompt_models::brain::{FieldDecl, FieldType, KindDecl, Materiality};

use crate::fixtures::kind;

fn field(alias: bool) -> FieldDecl {
    FieldDecl {
        field_type: FieldType::Text,
        required: false,
        max_len: None,
        entity_ref: None,
        values: Vec::new(),
        alias,
        display: None,
    }
}

fn contact() -> KindDecl {
    KindDecl {
        kind: kind("contact"),
        subject: "client".to_owned(),
        description: None,
        fields: BTreeMap::from([
            ("name".to_owned(), field(true)),
            ("legal_name".to_owned(), field(true)),
            ("note".to_owned(), field(false)),
        ]),
        topics: Vec::new(),
        statement: "{subject} is {name}".to_owned(),
        identity: vec!["subject".to_owned()],
        materiality: Materiality::default(),
        review_after_days: None,
        extractor: None,
        asks: Vec::new(),
    }
}

#[test]
fn aliases_come_from_the_id_the_display_name_and_alias_fields_once_each() {
    let fields = serde_json::json!({
        "name": "Acme Rockets",
        "legal_name": "Acme Rockets",
        "note": "not an alias",
    });
    let serde_json::Value::Object(fields) = fields else {
        unreachable!("object literal");
    };
    let aliases = derive_aliases(AliasInput {
        subject_id: "acme_rockets-ltd",
        display_name: Some("Acme"),
        kind: Some(&contact()),
        fields: &fields,
    });
    assert_eq!(
        aliases,
        vec![
            "acme_rockets-ltd".to_owned(),
            "acme rockets ltd".to_owned(),
            "Acme".to_owned(),
            "Acme Rockets".to_owned(),
        ]
    );
}

#[test]
fn an_undeclared_kind_blank_values_and_a_plain_id_yield_only_the_id() {
    let fields = serde_json::json!({ "name": "   ", "legal_name": 7 });
    let serde_json::Value::Object(fields) = fields else {
        unreachable!("object literal");
    };
    assert_eq!(
        derive_aliases(AliasInput {
            subject_id: "acme",
            display_name: None,
            kind: None,
            fields: &fields,
        }),
        vec!["acme".to_owned()],
        "no separators, no name, no kind: the id alone, once"
    );
    assert_eq!(
        derive_aliases(AliasInput {
            subject_id: "acme",
            display_name: Some(""),
            kind: Some(&contact()),
            fields: &fields,
        }),
        vec!["acme".to_owned(), "7".to_owned()],
        "a blank alias value is skipped and a number is its text"
    );
}
