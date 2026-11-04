//! The subject bridges a fact declares: a fact whose subject and one of
//! whose entity fields name two different subjects writes a
//! `brain_subject_links` row each way in the round that indexes it, and
//! erasing the fact removes both.

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_brain::plane::DeclaredBrain;
use systemprompt_brain::repository::{SubjectLink, SubjectLinkRepository};
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::{
    EnteredFact, EnteredStatus, FactScope, FieldDecl, FieldType, KindDecl, Materiality, SubjectKey,
};

use crate::harness::{Harness, Setup};
use crate::plane::{self, ENGINEERING, kind, topic};

pub(crate) const ENGAGEMENT: &str = "engagement";
pub(crate) const RISK: &str = "risk";
pub(crate) const CLIENT_NOTE: &str = "client_note";
pub(crate) const TEAM: &str = "team";
pub(crate) const PERSON: &str = "priya-shah";
pub(crate) const CLIENT: &str = "acme";
pub(crate) const PROJECT: &str = "cpq-rollout";

fn field(field_type: FieldType, entity_ref: Option<&str>, required: bool) -> FieldDecl {
    FieldDecl {
        field_type,
        required,
        max_len: Some(200),
        entity_ref: entity_ref.map(str::to_owned),
        values: Vec::new(),
        alias: false,
        display: None,
    }
}

fn decl(
    id: &str,
    subject: &str,
    fields: BTreeMap<String, FieldDecl>,
    statement: &str,
    identity: &[&str],
) -> KindDecl {
    KindDecl {
        kind: kind(id),
        subject: subject.to_owned(),
        description: None,
        fields,
        topics: vec![topic(ENGINEERING)],
        statement: statement.to_owned(),
        identity: identity.iter().map(|s| (*s).to_owned()).collect(),
        materiality: Materiality::default(),
        review_after_days: None,
        extractor: None,
        asks: Vec::new(),
    }
}

/// An engagement phase on a project naming its client (the bridge), a
/// delivery risk on a project naming no client, and a note on a client.
pub(crate) fn declared_with_bridges() -> DeclaredBrain {
    let mut declared = plane::declared();
    declared.kinds.push(decl(
        ENGAGEMENT,
        "project",
        BTreeMap::from([
            ("phase".to_owned(), field(FieldType::Text, None, true)),
            (
                "client".to_owned(),
                field(FieldType::Entity, Some("client"), false),
            ),
        ]),
        "{subject} is in {phase} for {client}",
        &["subject"],
    ));
    declared.kinds.push(decl(
        RISK,
        "project",
        BTreeMap::from([("risk".to_owned(), field(FieldType::Text, None, true))]),
        "Delivery concern on {subject}: {risk}",
        &["subject", "risk"],
    ));
    declared.kinds.push(decl(
        CLIENT_NOTE,
        "client",
        BTreeMap::from([("note".to_owned(), field(FieldType::Text, None, true))]),
        "{subject} prefers {note}",
        &["subject", "note"],
    ));
    let mut user = field(FieldType::Entity, Some("user"), false);
    user.display = Some("person".to_owned());
    declared.kinds.push(decl(
        TEAM,
        "project",
        BTreeMap::from([
            ("person".to_owned(), field(FieldType::Text, None, true)),
            ("user".to_owned(), user),
        ]),
        "{person} works on {subject}",
        &["subject", "person"],
    ));
    declared
}

/// A team fact on the project naming a person by display name and id.
pub(crate) async fn seed_person(h: &Harness, who: &UserId) {
    enter(
        h,
        who,
        TEAM,
        SubjectKey::new("project", PROJECT),
        serde_json::json!({ "person": "Priya Shah", "user": format!("user:{PERSON}") }),
    )
    .await;
}

pub(crate) async fn enter(
    h: &Harness,
    who: &UserId,
    kind_id: &str,
    subject: SubjectKey,
    fields: serde_json::Value,
) {
    let serde_json::Value::Object(fields) = fields else {
        panic!("fields must be an object");
    };
    let event = EnteredFact {
        kind: kind(kind_id),
        subject,
        fields,
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Proposed,
        observed_at: None,
    }
    .to_event(who.clone(), Utc::now())
    .expect("entered event");
    h.emit(&event).await;
    h.drain().await;
}

/// The engagement bridge, the risk and the client note; returns nothing,
/// the facts are read back by subject.
pub(crate) async fn seed_bridges(h: &Harness, who: &UserId) {
    enter(
        h,
        who,
        ENGAGEMENT,
        SubjectKey::new("project", PROJECT),
        serde_json::json!({ "phase": "build", "client": CLIENT }),
    )
    .await;
    enter(
        h,
        who,
        RISK,
        SubjectKey::new("project", PROJECT),
        serde_json::json!({ "risk": "data migration slips a sprint" }),
    )
    .await;
    enter(
        h,
        who,
        CLIENT_NOTE,
        SubjectKey::new("client", CLIENT),
        serde_json::json!({ "note": "fortnightly steering calls" }),
    )
    .await;
}

/// The one fact of `kind_id` about the subject.
pub(crate) async fn fact_of_kind(h: &Harness, subject: &str, kind_id: &str) -> BrainFactId {
    let mut found = Vec::new();
    for id in h.fact_ids(subject).await {
        if h.fact(&id).await.kind.as_str() == kind_id {
            found.push(id);
        }
    }
    assert_eq!(
        found.len(),
        1,
        "one {kind_id} fact about {subject}: {found:?}"
    );
    found.remove(0)
}

async fn bridges_of(h: &Harness, kind: &str, id: &str) -> Vec<SubjectLink> {
    SubjectLinkRepository::list_for(&h.pg, &SubjectKey::new(kind, id))
        .await
        .expect("subject links")
}

#[tokio::test]
async fn a_fact_naming_another_subject_bridges_both_ways_until_it_is_erased() {
    let h = Harness::with("brain_subject_links", Setup::confirming()).await;
    plane::apply(&h.pg, &declared_with_bridges()).await;
    let admin = h.user("admin", &["admin"]).await;
    seed_bridges(&h, &admin).await;
    let engagement = fact_of_kind(&h, PROJECT, ENGAGEMENT).await;

    assert_eq!(
        bridges_of(&h, "client", CLIENT).await,
        vec![SubjectLink {
            other: SubjectKey::new("project", PROJECT),
            via_kind: ENGAGEMENT.to_owned(),
            fact_id: engagement.to_string(),
        }],
        "the client reaches the project through the engagement fact"
    );
    assert_eq!(
        bridges_of(&h, "project", PROJECT).await,
        vec![SubjectLink {
            other: SubjectKey::new("client", CLIENT),
            via_kind: ENGAGEMENT.to_owned(),
            fact_id: engagement.to_string(),
        }],
        "and the project reaches the client; the risk, naming no client, bridges nothing"
    );
    assert_eq!(
        h.total("brain_subject_links").await,
        2,
        "one bridge, two directions"
    );

    h.service
        .erase(&Harness::caller(&admin, &["admin"]), &engagement)
        .await
        .expect("erase the bridge fact");
    assert!(bridges_of(&h, "client", CLIENT).await.is_empty());
    assert!(bridges_of(&h, "project", PROJECT).await.is_empty());
    assert_eq!(h.total("brain_subject_links").await, 0);
    h.finish().await;
}

#[tokio::test]
async fn an_entity_a_fact_names_becomes_a_subject_with_its_display_name_and_alias() {
    let h = Harness::with("brain_entity_subject", Setup::confirming()).await;
    plane::apply(&h.pg, &declared_with_bridges()).await;
    let admin = h.user("admin", &["admin"]).await;
    seed_person(&h, &admin).await;

    let display: Option<String> = sqlx::query_scalar(
        "SELECT display_name FROM brain_subjects WHERE subject_kind = 'user' AND subject_id = $1",
    )
    .bind(PERSON)
    .fetch_one(&h.pg)
    .await
    .expect("the person is a subject");
    assert_eq!(display.as_deref(), Some("Priya Shah"));
    let aliases: Vec<String> = sqlx::query_scalar(
        "SELECT alias_norm FROM brain_aliases WHERE subject_kind = 'user' AND subject_id = $1 ORDER BY 1",
    )
    .bind(PERSON)
    .fetch_all(&h.pg)
    .await
    .expect("aliases");
    assert_eq!(
        aliases,
        vec!["priya shah".to_owned(), "priya-shah".to_owned()]
    );
    h.finish().await;
}
