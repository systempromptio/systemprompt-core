//! `SubjectRef` — the tagged reference a rule binds to.
//!
//! Its serde tags are not presentation: they are the resolver's `rule_type`
//! vocabulary, and the stored rules on disk are written in them. A rename
//! here silently stops matching every rule already persisted, so the wire
//! spellings are asserted literally rather than round-tripped.

use systemprompt_identifiers::{DepartmentId, RoleId, UserId};
use systemprompt_security::authz::types::SubjectRef;

fn json(subject: &SubjectRef) -> serde_json::Value {
    serde_json::to_value(subject).expect("serialise subject")
}

#[test]
fn the_wire_tags_are_the_resolver_vocabulary() {
    assert_eq!(
        json(&SubjectRef::User(UserId::new("u1"))),
        serde_json::json!({"kind": "user", "id": "u1"})
    );
    assert_eq!(
        json(&SubjectRef::Department(DepartmentId::new("d1"))),
        serde_json::json!({"kind": "department", "id": "d1"})
    );
    assert_eq!(
        json(&SubjectRef::Role(RoleId::new("r1"))),
        serde_json::json!({"kind": "role", "id": "r1"})
    );
}

// Why: `rule_type` is what the resolver matches a stored rule's column
// against. If it disagreed with the serde tag, a rule would serialise under
// one dimension and be looked up under another — it would simply never match,
// with no error anywhere.
#[test]
fn rule_type_agrees_with_the_tag_it_serialises_under() {
    for subject in [
        SubjectRef::User(UserId::new("u1")),
        SubjectRef::Department(DepartmentId::new("d1")),
        SubjectRef::Role(RoleId::new("r1")),
    ] {
        let tag = json(&subject)["kind"].as_str().expect("tag").to_owned();
        assert_eq!(
            subject.rule_type().to_string(),
            tag,
            "{subject:?} serialises as {tag} but resolves as {}",
            subject.rule_type()
        );
    }
}

#[test]
fn value_returns_the_id_without_its_dimension() {
    assert_eq!(SubjectRef::User(UserId::new("u1")).value(), "u1");
    assert_eq!(
        SubjectRef::Department(DepartmentId::new("d1")).value(),
        "d1"
    );
    assert_eq!(SubjectRef::Role(RoleId::new("r1")).value(), "r1");
}

// Why: `Display` is what lands in audit lines. Without the dimension prefix a
// user and a role that share an id read identically in the record.
#[test]
fn display_carries_the_dimension_alongside_the_id() {
    assert_eq!(
        SubjectRef::User(UserId::new("shared")).to_string(),
        "user:shared"
    );
    assert_ne!(
        SubjectRef::Role(RoleId::new("shared")).to_string(),
        SubjectRef::User(UserId::new("shared")).to_string(),
        "a role and a user with the same id must not be indistinguishable"
    );
}

#[test]
fn a_serialised_subject_reads_back_as_the_same_subject() {
    for subject in [
        SubjectRef::User(UserId::new("u1")),
        SubjectRef::Department(DepartmentId::new("d1")),
        SubjectRef::Role(RoleId::new("r1")),
    ] {
        let back: SubjectRef = serde_json::from_value(json(&subject)).expect("round trip");
        assert_eq!(back, subject);
    }
}
