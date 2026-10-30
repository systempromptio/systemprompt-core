use serde_json::{Map, Value, json};
use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{
    StatementError, SubjectKey, identity_hash, placeholders, render_statement, statement_hash,
};

const TEMPLATE: &str = "{party} must {action}{due? by {due}}";

fn fields(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn subject() -> SubjectKey {
    SubjectKey::new("matter", "m-42")
}

#[test]
fn renders_optional_group_when_present() {
    let rendered = render_statement(
        TEMPLATE,
        &subject(),
        &fields(json!({"party": "Acme", "action": "deliver", "due": "2026-12-01"})),
    );
    assert_eq!(rendered, Ok("Acme must deliver by 2026-12-01".to_owned()));
}

#[test]
fn omits_optional_group_when_absent_or_empty() {
    for due in [Value::Null, json!(""), json!("   ")] {
        let rendered = render_statement(
            TEMPLATE,
            &subject(),
            &fields(json!({"party": "Acme", "action": "deliver", "due": due})),
        );
        assert_eq!(rendered, Ok("Acme must deliver".to_owned()));
    }
    let rendered = render_statement(
        TEMPLATE,
        &subject(),
        &fields(json!({"party": "Acme", "action": "deliver"})),
    );
    assert_eq!(rendered, Ok("Acme must deliver".to_owned()));
}

#[test]
fn missing_required_field_is_an_error() {
    let rendered = render_statement(TEMPLATE, &subject(), &fields(json!({"party": "Acme"})));
    assert_eq!(
        rendered,
        Err(StatementError::MissingField("action".to_owned()))
    );
}

#[test]
fn subject_placeholder_renders_subject_id() {
    let rendered = render_statement(
        "{subject} is {state}",
        &subject(),
        &fields(json!({"state": 3})),
    );
    assert_eq!(rendered, Ok("m-42 is 3".to_owned()));
}

#[test]
fn placeholders_lists_every_name() {
    assert_eq!(
        placeholders(TEMPLATE),
        Ok(vec![
            "party".to_owned(),
            "action".to_owned(),
            "due".to_owned(),
            "due".to_owned()
        ])
    );
}

#[test]
fn malformed_templates_are_refused() {
    assert!(matches!(
        placeholders("{Party}"),
        Err(StatementError::BadPlaceholder(_))
    ));
    assert!(matches!(
        placeholders("{}"),
        Err(StatementError::BadPlaceholder(_))
    ));
    assert!(matches!(
        placeholders("{party"),
        Err(StatementError::Unbalanced(_))
    ));
    assert!(matches!(
        placeholders("party}"),
        Err(StatementError::Unbalanced(_))
    ));
    assert!(matches!(
        placeholders("{due? by {due}"),
        Err(StatementError::Unbalanced(_))
    ));
}

#[test]
fn identity_hash_is_stable_and_field_order_independent() {
    let kind = FactKindId::new("obligation");
    let identity = vec![
        "subject".to_owned(),
        "party".to_owned(),
        "clause".to_owned(),
    ];
    let a = fields(json!({"party": "Acme", "clause": "4.2", "action": "pay"}));
    let b = fields(json!({"action": "ship", "clause": "4.2", "party": "  ACME "}));
    let reordered = vec![
        "clause".to_owned(),
        "party".to_owned(),
        "subject".to_owned(),
    ];
    let first = identity_hash(&kind, &subject(), &identity, &a);
    assert_eq!(first, identity_hash(&kind, &subject(), &identity, &a));
    assert_eq!(first, identity_hash(&kind, &subject(), &identity, &b));
    assert_eq!(first, identity_hash(&kind, &subject(), &reordered, &a));
    assert_eq!(first.len(), 64);
}

#[test]
fn identity_hash_changes_with_identity_values() {
    let kind = FactKindId::new("obligation");
    let identity = vec!["subject".to_owned(), "clause".to_owned()];
    let a = fields(json!({"clause": "4.2"}));
    let b = fields(json!({"clause": "4.3"}));
    assert_ne!(
        identity_hash(&kind, &subject(), &identity, &a),
        identity_hash(&kind, &subject(), &identity, &b)
    );
    assert_ne!(
        identity_hash(&kind, &subject(), &identity, &a),
        identity_hash(&FactKindId::new("decision"), &subject(), &identity, &a)
    );
    assert_ne!(
        identity_hash(&kind, &subject(), &identity, &a),
        identity_hash(&kind, &SubjectKey::new("matter", "m-43"), &identity, &a)
    );
}

#[test]
fn statement_hash_normalises_case_and_whitespace() {
    assert_eq!(
        statement_hash("Acme  must pay"),
        statement_hash(" acme must\tPAY ")
    );
    assert_ne!(
        statement_hash("Acme must pay"),
        statement_hash("Acme must ship")
    );
}
