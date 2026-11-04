//! The derived indexes a fact keeps current: links from its subject and
//! entity fields, aliases for its subject, the weighted search vector with
//! its synonym expansions, and the observed_at instant behind as-of reads.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use systemprompt_brain::models::FactFilter;
use systemprompt_brain::plane::Rebuilds;
use systemprompt_brain::repository::{FactLinkRepository, SearchIndexRepository};
use systemprompt_models::brain::{
    EnteredFact, EnteredStatus, FactScope, FieldDecl, FieldType, KindDecl, Materiality, SubjectKey,
    SynonymsDoc,
};

use crate::harness::{Harness, Setup};
use systemprompt_brain::plane::DeclaredBrain;

use crate::plane::{self, FINANCE, kind, topic};

pub(crate) const CONTACT: &str = "contact";

/// The highest weight a lexeme carries in a tsvector's text form
/// (`'lexeme':3A,7B`); `None` when the lexeme is absent or unweighted.
fn weight_of(vector: &str, lexeme: &str) -> Option<char> {
    let needle = format!("'{lexeme}':");
    let start = vector.find(&needle)? + needle.len();
    let positions = vector[start..].split(' ').next().unwrap_or_default();
    positions
        .chars()
        .filter(|c| matches!(c, 'A' | 'B' | 'C' | 'D'))
        .min()
}

fn field(field_type: FieldType, entity_ref: Option<&str>, alias: bool) -> FieldDecl {
    FieldDecl {
        field_type,
        required: false,
        max_len: Some(200),
        entity_ref: entity_ref.map(str::to_owned),
        values: Vec::new(),
        alias,
    }
}

pub(crate) fn contact_kind() -> KindDecl {
    KindDecl {
        kind: kind(CONTACT),
        subject: "client".to_owned(),
        description: None,
        fields: BTreeMap::from([
            ("name".to_owned(), field(FieldType::Text, None, true)),
            (
                "account".to_owned(),
                field(FieldType::Entity, Some("client"), false),
            ),
            ("owner".to_owned(), field(FieldType::Entity, None, false)),
            ("note".to_owned(), field(FieldType::Text, None, false)),
        ]),
        topics: vec![topic(FINANCE)],
        statement: "{subject} is reached through {name}{note? ({note})}".to_owned(),
        identity: vec!["subject".to_owned(), "name".to_owned()],
        materiality: Materiality::default(),
        review_after_days: None,
        extractor: None,
    }
}

pub(crate) fn declared_with_contact() -> DeclaredBrain {
    let mut declared = plane::declared();
    declared.kinds.push(contact_kind());
    declared.synonyms = Some(SynonymsDoc {
        groups: vec![vec![
            "invoice".to_owned(),
            "bill".to_owned(),
            "statement of account".to_owned(),
        ]],
    });
    declared
}

async fn enter_contact(h: &Harness, who: &systemprompt_identifiers::UserId) -> EnteredFact {
    let fact = EnteredFact {
        kind: kind(CONTACT),
        subject: SubjectKey::new("client", "acme-rockets"),
        fields: serde_json::Map::from_iter([
            ("name".to_owned(), serde_json::json!("Jane Doe")),
            ("account".to_owned(), serde_json::json!("acme-holdings")),
            ("owner".to_owned(), serde_json::json!("user:u_42")),
            (
                "note".to_owned(),
                serde_json::json!("sends the bill monthly"),
            ),
        ]),
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Proposed,
        observed_at: Some(Utc::now() - Duration::days(30)),
    };
    let event = fact
        .to_event(who.clone(), Utc::now())
        .expect("entered event");
    h.emit(&event).await;
    h.drain().await;
    fact
}

#[tokio::test]
async fn links_aliases_and_weighted_search_are_written_with_the_fact() {
    let h = Harness::with("brain_index_write", Setup::confirming()).await;
    plane::apply(&h.pg, &declared_with_contact()).await;
    let admin = h.user("admin", &["admin"]).await;
    enter_contact(&h, &admin).await;
    let fact = h.only_fact("acme-rockets").await;

    let links = FactLinkRepository::list_for(&h.pg, &fact.id)
        .await
        .expect("links");
    let shown: Vec<(String, String)> = links
        .iter()
        .map(|l| (l.via_field.clone(), l.subject.to_string()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("account".to_owned(), "client:acme-holdings".to_owned()),
            ("owner".to_owned(), "user:u_42".to_owned()),
            ("subject".to_owned(), "client:acme-rockets".to_owned()),
        ],
        "a bare entity value takes the field's ref kind; a kind:id value is split"
    );

    let aliases: Vec<String> = sqlx::query_scalar(
        "SELECT alias FROM brain_aliases WHERE subject_kind = 'client' AND subject_id = $1 \
         ORDER BY alias COLLATE \"C\"",
    )
    .bind("acme-rockets")
    .fetch_all(&h.pg)
    .await
    .expect("aliases");
    assert_eq!(
        aliases,
        vec![
            "Jane Doe".to_owned(),
            "acme rockets".to_owned(),
            "acme-rockets".to_owned()
        ],
        "the id, the id with separators as spaces, and the alias field"
    );

    let vector = SearchIndexRepository::vector_text(&h.pg, &fact.id)
        .await
        .expect("vector")
        .expect("fact exists");
    assert_eq!(weight_of(&vector, "jane"), Some('A'), "{vector}");
    assert_eq!(weight_of(&vector, "doe"), Some('A'), "{vector}");
    assert_eq!(weight_of(&vector, "acme-rockets"), Some('A'), "{vector}");
    assert_eq!(weight_of(&vector, "monthly"), Some('B'), "{vector}");
    assert_eq!(weight_of(&vector, "acme-holdings"), Some('B'), "{vector}");
    assert_eq!(weight_of(&vector, "reached"), Some('C'), "{vector}");
    assert_eq!(
        weight_of(&vector, "invoice"),
        Some('C'),
        "every member of the matched synonym group is indexed: {vector}"
    );
    assert_eq!(weight_of(&vector, "account"), Some('C'), "{vector}");
    h.finish().await;
}

#[tokio::test]
async fn a_synonyms_change_rebuilds_every_vector_and_as_of_reads_follow_observed_at() {
    let h = Harness::with("brain_index_rebuild", Setup::confirming()).await;
    let mut declared = declared_with_contact();
    declared.synonyms = None;
    plane::apply(&h.pg, &declared).await;
    let admin = h.user("admin", &["admin"]).await;
    let entered = enter_contact(&h, &admin).await;
    let fact = h.only_fact("acme-rockets").await;
    let before = SearchIndexRepository::vector_text(&h.pg, &fact.id)
        .await
        .expect("vector")
        .expect("fact exists");
    assert!(!before.contains("'invoice':"), "{before}");

    plane::apply(&h.pg, &declared_with_contact()).await;
    let after = SearchIndexRepository::vector_text(&h.pg, &fact.id)
        .await
        .expect("vector")
        .expect("fact exists");
    assert!(after.contains("'invoice':"), "{after}");
    assert_eq!(
        Rebuilds::for_keys(["doc/synonyms", "kind/contact"]),
        Rebuilds {
            working_set: false,
            links: true,
            search: true,
        }
    );

    let observed = entered.observed_at.expect("entered with observed_at");
    assert_eq!(
        fact.observed_at.timestamp_millis(),
        observed.timestamp_millis(),
        "the entered instant is stored"
    );
    let caller = Harness::caller(&admin, &["admin"]);
    let seen_then = h
        .service
        .search(
            &caller,
            &FactFilter::new()
                .with_kind(kind(CONTACT))
                .with_as_of(observed + Duration::hours(1)),
        )
        .await
        .expect("as of after");
    assert_eq!(seen_then.len(), 1);
    let unseen = h
        .service
        .search(
            &caller,
            &FactFilter::new()
                .with_kind(kind(CONTACT))
                .with_as_of(observed - Duration::hours(1)),
        )
        .await
        .expect("as of before");
    assert!(unseen.is_empty(), "{unseen:?}");
    h.finish().await;
}
