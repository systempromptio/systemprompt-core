use systemprompt_models::brain::{BrainDeclError, BrainDocs, KindDecl, TopicDecl, validate};

const KIND: &str = r#"
kind: obligation
subject: matter
fields:
  party: { type: entity, ref: party, required: true }
  action: { type: text, max_len: 200, required: true }
  due: { type: date }
  clause: { type: text, max_len: 20 }
topics: [matter.obligations]
statement: "{party} must {action}{due? by {due}}"
identity: [subject, party, clause]
materiality: { require: [due] }
review_after_days: 180
extractor: { type: model, prompt: "Extract obligations.", prompt_version: "v1" }
"#;

fn kind() -> KindDecl {
    serde_yaml::from_str(KIND).unwrap_or_else(|e| panic!("kind yaml: {e}"))
}

fn docs() -> BrainDocs {
    let topic: TopicDecl = serde_yaml::from_str("id: matter.obligations\nname: Obligations\n")
        .unwrap_or_else(|e| panic!("topic yaml: {e}"));
    let mut docs = BrainDocs::default();
    docs.topics.topics.push(topic);
    docs.kinds.push(kind());
    docs
}

fn with_kind(edit: impl FnOnce(&mut KindDecl)) -> Result<(), BrainDeclError> {
    let mut docs = docs();
    if let Some(kind) = docs.kinds.first_mut() {
        edit(kind);
    }
    validate(&docs)
}

#[test]
fn reference_kind_validates() {
    assert_eq!(validate(&docs()), Ok(()));
}

#[test]
fn unknown_topic_is_refused() {
    let result = with_kind(|kind| {
        kind.topics
            .push(systemprompt_identifiers::TopicId::new("nope"))
    });
    assert!(matches!(result, Err(BrainDeclError::UnknownTopic { .. })));
}

#[test]
fn bad_placeholder_is_refused() {
    let result = with_kind(|kind| kind.statement = "{party} must {Action}".to_owned());
    assert!(matches!(result, Err(BrainDeclError::Statement { .. })));
    let result = with_kind(|kind| kind.statement = "{party} must {penalty}".to_owned());
    assert!(matches!(
        result,
        Err(BrainDeclError::UnknownPlaceholder { .. })
    ));
}

#[test]
fn identity_outside_fields_is_refused() {
    let result = with_kind(|kind| kind.identity.push("counterparty".to_owned()));
    assert!(matches!(
        result,
        Err(BrainDeclError::UnknownIdentityField { .. })
    ));
    let result = with_kind(|kind| kind.identity.clear());
    assert!(matches!(result, Err(BrainDeclError::EmptyIdentity(_))));
}

#[test]
fn materiality_outside_fields_is_refused() {
    let result = with_kind(|kind| kind.materiality.require.push("subject".to_owned()));
    assert!(matches!(
        result,
        Err(BrainDeclError::UnknownMaterialityField { .. })
    ));
}

#[test]
fn model_extractor_without_prompt_version_is_refused() {
    let yaml = KIND.replace(", prompt_version: \"v1\"", "");
    let kind: KindDecl = serde_yaml::from_str(&yaml).unwrap_or_else(|e| panic!("yaml: {e}"));
    let result = with_kind(|slot| *slot = kind);
    assert!(matches!(
        result,
        Err(BrainDeclError::MissingPromptVersion(_))
    ));
}

#[test]
fn weight_out_of_range_is_refused() {
    for value in [1.5, -0.1, f64::NAN] {
        let mut docs = docs();
        docs.importance.in_context.subject_match = value;
        assert!(matches!(
            validate(&docs),
            Err(BrainDeclError::WeightOutOfRange { .. })
        ));
    }
    let mut docs = docs();
    docs.retrieval.tiers.context = 2.0;
    assert!(matches!(
        validate(&docs),
        Err(BrainDeclError::WeightOutOfRange { .. })
    ));
}

#[test]
fn duplicate_kind_is_refused() {
    let mut docs = docs();
    docs.kinds.push(kind());
    assert!(matches!(
        validate(&docs),
        Err(BrainDeclError::Duplicate { .. })
    ));
}

#[test]
fn unknown_field_in_kind_yaml_is_refused() {
    let yaml = format!("{KIND}colour: red\n");
    assert!(serde_yaml::from_str::<KindDecl>(&yaml).is_err());
}
