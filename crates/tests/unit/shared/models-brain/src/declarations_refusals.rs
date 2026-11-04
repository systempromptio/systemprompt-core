use std::collections::BTreeMap;

use systemprompt_identifiers::{FactKindId, WorkflowId};
use systemprompt_models::brain::{
    BrainDeclError, BrainDocs, InterestDecl, KindDecl, MAX_SUBJECT_LIMIT, TopicDecl,
    WorkflowProfileDecl, validate,
};

const KIND: &str = r#"
kind: obligation
subject: matter
fields:
  party: { type: entity, ref: party, required: true }
  action: { type: text, max_len: 200, required: true }
topics: [matter.obligations]
statement: "{party} must {action}"
identity: [subject, party]
"#;

fn topic() -> TopicDecl {
    serde_yaml::from_str("id: matter.obligations\nname: Obligations\n").expect("topic")
}

fn docs() -> BrainDocs {
    let mut docs = BrainDocs::default();
    docs.topics.topics.push(topic());
    docs.kinds
        .push(serde_yaml::from_str::<KindDecl>(KIND).expect("kind"));
    docs
}

fn interest(id: &str, kinds: &str) -> InterestDecl {
    serde_yaml::from_str(&format!(
        "id: {id}\nname: Watch\nowner: user-brain\nquestion: What changed?\nmatch: {{ kinds: \
         [{kinds}] }}\n"
    ))
    .expect("interest")
}

fn workflow(id: &str, kinds: &str) -> WorkflowProfileDecl {
    serde_yaml::from_str(&format!("id: {id}\nname: Flow\nkinds: [{kinds}]\n")).expect("workflow")
}

fn unknown_kind(docs: &BrainDocs) -> (String, String) {
    match validate(docs) {
        Err(BrainDeclError::UnknownKind { owner, kind }) => (owner, kind.as_str().to_owned()),
        other => panic!("expected UnknownKind, got {other:?}"),
    }
}

fn duplicate_of(docs: &BrainDocs) -> &'static str {
    match validate(docs) {
        Err(BrainDeclError::Duplicate { what, .. }) => what,
        other => panic!("expected Duplicate, got {other:?}"),
    }
}

#[test]
fn duplicate_topics_interests_and_workflows_are_refused() {
    let mut topics = docs();
    topics.topics.topics.push(topic());
    assert_eq!(duplicate_of(&topics), "topic");

    let mut interests = docs();
    interests.interests = vec![
        interest("watch-a", "obligation"),
        interest("watch-a", "obligation"),
    ];
    assert_eq!(duplicate_of(&interests), "interest");

    let mut workflows = docs();
    workflows.workflows = vec![
        workflow("flow-a", "obligation"),
        workflow("flow-a", "obligation"),
    ];
    assert_eq!(duplicate_of(&workflows), "workflow");
}

#[test]
fn declared_references_validate() {
    let mut ok = docs();
    ok.interests = vec![interest("watch-ok", "obligation")];
    ok.workflows = vec![workflow("flow-ok", "obligation")];
    assert_eq!(validate(&ok), Ok(()));
}

#[test]
fn an_interest_naming_an_undeclared_kind_is_refused() {
    let mut docs = docs();
    docs.interests = vec![interest("watch-ghost", "ghost")];
    let (owner, kind) = unknown_kind(&docs);
    assert_eq!(owner, "interest watch-ghost");
    assert_eq!(kind, "ghost");
}

#[test]
fn a_workflow_naming_an_undeclared_kind_is_refused() {
    let mut docs = docs();
    docs.workflows = vec![workflow("flow-ghost", "ghost")];
    let (owner, _) = unknown_kind(&docs);
    assert_eq!(owner, "workflow flow-ghost");
}

#[test]
fn importance_and_retrieval_quotas_must_name_declared_kinds() {
    let mut importance = docs();
    importance
        .importance
        .base
        .kind_weight
        .insert(FactKindId::new("ghost"), 0.5);
    assert_eq!(unknown_kind(&importance).0, "importance");

    let mut retrieval = docs();
    retrieval
        .retrieval
        .quotas
        .insert(FactKindId::new("ghost"), 3);
    assert_eq!(unknown_kind(&retrieval).0, "retrieval");
}

#[test]
fn out_of_range_importance_weights_are_named() {
    let mut kind_weight = docs();
    kind_weight
        .importance
        .base
        .kind_weight
        .insert(FactKindId::new("obligation"), 1.2);
    assert!(matches!(
        validate(&kind_weight),
        Err(BrainDeclError::WeightOutOfRange { ref name, .. }) if name == "kind_weight.obligation"
    ));

    let mut owner = docs();
    owner.importance.base.confirmed_by_owner = -0.5;
    assert!(matches!(
        validate(&owner),
        Err(BrainDeclError::WeightOutOfRange { ref name, .. }) if name == "confirmed_by_owner"
    ));

    let mut flows = docs();
    flows.importance.in_context.workflow_weight.insert(
        WorkflowId::new("flow-w"),
        BTreeMap::from([(FactKindId::new("obligation"), 3.0)]),
    );
    assert!(matches!(
        validate(&flows),
        Err(BrainDeclError::WeightOutOfRange { ref name, .. })
            if name == "workflow_weight.flow-w.obligation"
    ));
}

#[test]
fn a_zero_recency_half_life_is_refused() {
    let mut docs = docs();
    docs.importance.in_context.recency.half_life_days = 0;
    assert_eq!(validate(&docs), Err(BrainDeclError::ZeroHalfLife));
}

#[test]
fn a_subject_limit_over_the_maximum_is_refused() {
    let mut docs = docs();
    docs.retrieval.subject_limit = MAX_SUBJECT_LIMIT;
    assert_eq!(validate(&docs), Ok(()));
    docs.retrieval.subject_limit = MAX_SUBJECT_LIMIT + 1;
    assert_eq!(
        validate(&docs),
        Err(BrainDeclError::LimitOverMax {
            name: "subject_limit",
            value: MAX_SUBJECT_LIMIT + 1,
            max: MAX_SUBJECT_LIMIT,
        })
    );
}

#[test]
fn a_stop_word_must_be_one_lower_case_token() {
    let mut docs = docs();
    docs.retrieval.search_stop_words = vec!["call".to_owned(), "the".to_owned()];
    assert_eq!(validate(&docs), Ok(()));
    for bad in ["", "What", "what's", "two words"] {
        docs.retrieval.search_stop_words = vec![bad.to_owned()];
        assert_eq!(
            validate(&docs),
            Err(BrainDeclError::StopWordNotToken {
                word: bad.to_owned()
            }),
            "{bad:?}"
        );
    }
}
