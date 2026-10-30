use std::collections::HashSet;

use systemprompt_brain::defaults::SchemaValidator;
use systemprompt_extension::brain::ValidationVerdict;
use systemprompt_identifiers::TopicId;
use systemprompt_models::brain::{FactCandidate, KindDecl, SourceText};

use crate::fixtures::{subject, topic};
use crate::pipeline_fixtures::{kind_decl, source_text, span, span_source, valid_candidate};

fn topics() -> HashSet<TopicId> {
    [topic("ops")].into_iter().collect()
}

fn verdict(candidate: FactCandidate, kind: &KindDecl, text: Option<&SourceText>) -> ValidationVerdict {
    SchemaValidator::check(candidate, kind, text, &topics()).verdict
}

fn with_field(name: &str, value: serde_json::Value) -> FactCandidate {
    let mut candidate = valid_candidate();
    candidate.fields.insert(name.to_owned(), value);
    candidate
}

fn rejected(verdict: &ValidationVerdict) -> bool {
    matches!(verdict, ValidationVerdict::Reject { .. })
}

#[test]
fn a_well_formed_candidate_is_accepted() {
    assert_eq!(verdict(valid_candidate(), &kind_decl(), None), ValidationVerdict::Accept);
}

#[test]
fn each_field_rule_rejects() {
    let kind = kind_decl();
    let cases = [
        ("owner", serde_json::Value::Null),
        ("owner", serde_json::json!("much too long a name")),
        ("status", serde_json::json!("pending")),
        ("due", serde_json::json!("next tuesday")),
        ("amount", serde_json::json!("twelve")),
        ("partner", serde_json::json!("vendor:initech")),
        ("unknown", serde_json::json!("x")),
    ];
    for (name, value) in cases {
        let outcome = verdict(with_field(name, value.clone()), &kind, None);
        assert!(rejected(&outcome), "{name}={value} should be rejected");
    }
}

#[test]
fn a_subject_of_another_kind_is_rejected() {
    let mut candidate = valid_candidate();
    candidate.subject = subject("vendor:acme");
    assert!(rejected(&verdict(candidate, &kind_decl(), None)));
}

#[test]
fn a_cited_span_must_exist_in_the_source() {
    let kind = kind_decl();
    let text = source_text();
    let mut cited = valid_candidate();
    cited.source = span_source(Some(span("n1", "abcd1234", "owes billing")));
    assert_eq!(verdict(cited.clone(), &kind, Some(&text)), ValidationVerdict::Accept);
    assert!(rejected(&verdict(cited, &kind, None)));
    let mut wrong_hash = valid_candidate();
    wrong_hash.source = span_source(Some(span("n1", "ffffffff", "owes billing")));
    assert!(rejected(&verdict(wrong_hash, &kind, Some(&text))));
    let mut missing = valid_candidate();
    missing.source = span_source(Some(span("n1", "abcd1234", "not in the node")));
    assert!(rejected(&verdict(missing, &kind, Some(&text))));
}

#[test]
fn an_unknown_topic_is_dropped_not_rejected() {
    let mut candidate = valid_candidate();
    candidate.topics = vec![topic("ops"), topic("nobody-declared")];
    let check = SchemaValidator::check(candidate, &kind_decl(), None, &topics());
    assert_eq!(check.verdict, ValidationVerdict::Accept);
    assert_eq!(check.candidate.topics, vec![topic("ops")]);
    assert_eq!(check.dropped_topics, vec![topic("nobody-declared")]);
}

#[test]
fn a_permissive_kind_still_rejects_a_missing_required_field() {
    let mut kind = kind_decl();
    kind.materiality.require.clear();
    for (name, field) in &mut kind.fields {
        field.required = name == "owner";
        field.max_len = None;
        field.entity_ref = None;
    }
    let mut candidate = valid_candidate();
    candidate.fields.remove("owner");
    assert!(rejected(&verdict(candidate, &kind, None)));
}
