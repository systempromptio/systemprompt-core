//! Person entry refuses an erased identity and a subject the store does
//! not admit before anything is queued, and a field edit merges onto the
//! stored fields: omitted keys are kept, the merged map is checked against
//! the kind, and an identity change is refused.

use serde_json::{Map, Value, json};
use systemprompt_brain::BrainError;
use systemprompt_identifiers::UserId;
use systemprompt_models::brain::{
    EnteredFact, EnteredStatus, FactScope, SubjectKey, SubjectKeyError,
};

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::plane::{PRICING, kind};

const OUTBOX: &str = "SELECT COUNT(*) FROM event_outbox WHERE consumer = 'brain' AND actor_id = $1";

fn fields(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
}

fn entered(owner: &UserId, region: &str) -> EnteredFact {
    EnteredFact {
        kind: kind(PRICING),
        subject: SubjectKey::new("product", "acme"),
        fields: fields(&[("price", json!("1200")), ("region", json!(region))]),
        scope: FactScope::User(owner.clone()),
        topics: Vec::new(),
        status: EnteredStatus::Candidate,
        observed_at: None,
    }
}

#[tokio::test]
async fn entering_an_oversized_subject_is_refused_before_it_is_queued() {
    let h = Harness::new("brain_enter_oversized").await;
    let owner = h.user("owner", &[]).await;
    let caller = Harness::caller(&owner, &[]);
    let mut fact = entered(&owner, "emea");
    fact.subject = SubjectKey::new("product", "x".repeat(300));

    let refused = h
        .service
        .enter(&caller, &fact)
        .await
        .expect_err("a subject brain_subjects cannot hold is refused at entry");
    assert!(
        matches!(
            refused,
            BrainError::Subject(SubjectKeyError::IdLength { len: 300 })
        ),
        "{refused:?}"
    );
    assert_eq!(h.count(OUTBOX, owner.as_str()).await, 0, "nothing queued");

    fact.subject = SubjectKey::new("k".repeat(65), "acme");
    let refused = h
        .service
        .enter(&caller, &fact)
        .await
        .expect_err("an oversized subject kind is refused too");
    assert!(
        matches!(
            refused,
            BrainError::Subject(SubjectKeyError::KindLength { len: 65 })
        ),
        "{refused:?}"
    );
    assert_eq!(h.count(OUTBOX, owner.as_str()).await, 0);
    h.finish().await;
}

#[tokio::test]
async fn entering_an_erased_identity_is_refused_before_it_is_queued() {
    let h = Harness::new("brain_enter_tombstone").await;
    let owner = h.user("owner", &[]).await;
    h.model.push(json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let fact = h.only_fact("acme").await;
    let caller = Harness::caller(&owner, &[]);
    h.service.erase(&caller, &fact.id).await.expect("erase");
    let queued = h.count(OUTBOX, owner.as_str()).await;

    let refused = h
        .service
        .enter(&caller, &entered(&owner, "emea"))
        .await
        .expect_err("the erased identity is refused at entry");
    assert!(matches!(refused, BrainError::Tombstoned), "{refused:?}");
    assert_eq!(
        h.count(OUTBOX, owner.as_str()).await,
        queued,
        "nothing queued"
    );

    h.service
        .enter(&caller, &entered(&owner, "apac"))
        .await
        .expect("negative control: another identity of the kind is entered");
    h.finish().await;
}

#[tokio::test]
async fn a_field_edit_merges_onto_the_stored_fields() {
    let h = Harness::new("brain_edit_merge").await;
    let owner = h.user("owner", &[]).await;
    h.model.push(json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let fact = h.only_fact("acme").await;
    let caller = Harness::caller(&owner, &[]);

    h.service
        .edit_fields(&caller, &fact.id, fields(&[("price", json!("1500"))]))
        .await
        .expect("a partial edit of a non-identity field");
    let edited = h.fact(&fact.id).await;
    assert_eq!(edited.fields.get("price"), Some(&json!("1500")));
    assert_eq!(edited.fields.get("region"), Some(&json!("emea")), "kept");
    assert!(edited.statement.contains("1500"), "{}", edited.statement);
    assert_eq!(edited.identity_hash, fact.identity_hash);

    let moved = h
        .service
        .edit_fields(&caller, &fact.id, fields(&[("region", json!("apac"))]))
        .await
        .expect_err("an identity change is refused");
    assert!(matches!(moved, BrainError::Forbidden { .. }), "{moved:?}");

    for bad in [
        fields(&[("price", Value::Null)]),
        fields(&[("undeclared", json!("x"))]),
        fields(&[("price", json!(7))]),
    ] {
        let refused = h
            .service
            .edit_fields(&caller, &fact.id, bad.clone())
            .await
            .expect_err("a merged map that does not fit the kind");
        assert!(
            matches!(refused, BrainError::InvalidFields(_)),
            "{bad:?}: {refused:?}"
        );
    }
    assert_eq!(
        h.fact(&fact.id).await.fields.get("price"),
        Some(&json!("1500"))
    );
    h.finish().await;
}
