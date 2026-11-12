use systemprompt_brain::BrainError;
use systemprompt_brain::service::{DisputeOutcome, DisputeResolution};
use systemprompt_identifiers::{BrainDisputeId, BrainFactId, UserId};
use systemprompt_models::brain::{
    BrainFact, EnteredFact, EnteredStatus, FactScope, FactStatus, SubjectKey,
};

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::plane::{OWNERSHIP, kind};

async fn captured(h: &Harness, product: &str) -> (UserId, BrainFact) {
    let owner = h.user("owner", &[]).await;
    h.model
        .push(serde_json::json!([price(product, "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let fact = h.only_fact(product).await;
    (owner, fact)
}

fn resolution(
    fact: &BrainFactId,
    dispute: &BrainDisputeId,
    outcome: DisputeOutcome,
) -> DisputeResolution {
    DisputeResolution {
        fact_id: fact.clone(),
        dispute_id: dispute.clone(),
        outcome,
        note: None,
    }
}

async fn open_disputes(h: &Harness, fact: &BrainFactId) -> i64 {
    h.count(
        "SELECT COUNT(*) FROM brain_disputes WHERE fact_id = $1 AND status = 'open'",
        fact.as_str(),
    )
    .await
}

#[tokio::test]
async fn dismissing_the_only_dispute_restores_the_fact_to_confirmed() {
    let h = Harness::new("brain_disputes_dismiss").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    let dispute = h
        .service
        .dispute(&caller, &fact.id, "looks stale")
        .await
        .expect("dispute");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Disputed);
    assert_eq!(open_disputes(&h, &fact.id).await, 1);

    h.service
        .resolve_dispute(
            &caller,
            &resolution(&fact.id, &dispute, DisputeOutcome::Dismissed),
        )
        .await
        .expect("dismiss");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Confirmed);
    assert_eq!(open_disputes(&h, &fact.id).await, 0);
    h.finish().await;
}

#[tokio::test]
async fn dismissing_one_of_two_open_disputes_keeps_the_fact_disputed() {
    let h = Harness::new("brain_disputes_two").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    let first = h
        .service
        .dispute(&caller, &fact.id, "first")
        .await
        .expect("first dispute");
    let second = h
        .service
        .dispute(&caller, &fact.id, "second")
        .await
        .expect("second dispute");
    assert_ne!(first, second);
    assert_eq!(open_disputes(&h, &fact.id).await, 2);

    h.service
        .resolve_dispute(
            &caller,
            &resolution(&fact.id, &first, DisputeOutcome::Dismissed),
        )
        .await
        .expect("dismiss first");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Disputed);
    assert_eq!(open_disputes(&h, &fact.id).await, 1);

    h.service
        .resolve_dispute(
            &caller,
            &resolution(&fact.id, &second, DisputeOutcome::Dismissed),
        )
        .await
        .expect("dismiss second");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Confirmed);
    h.finish().await;
}

#[tokio::test]
async fn resolving_a_dispute_twice_or_one_not_on_the_fact_is_refused() {
    let h = Harness::new("brain_disputes_refused").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    let dispute = h
        .service
        .dispute(&caller, &fact.id, "wrong")
        .await
        .expect("dispute");

    let error = h
        .service
        .resolve_dispute(
            &caller,
            &resolution(
                &fact.id,
                &BrainDisputeId::generate(),
                DisputeOutcome::Upheld,
            ),
        )
        .await
        .expect_err("an unknown dispute");
    assert!(matches!(error, BrainError::Forbidden { .. }), "{error:?}");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Disputed);

    h.service
        .resolve_dispute(
            &caller,
            &resolution(&fact.id, &dispute, DisputeOutcome::Upheld),
        )
        .await
        .expect("uphold");
    let error = h
        .service
        .resolve_dispute(
            &caller,
            &resolution(&fact.id, &dispute, DisputeOutcome::Dismissed),
        )
        .await
        .expect_err("already resolved");
    assert!(matches!(error, BrainError::Forbidden { .. }), "{error:?}");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Retired);
    h.finish().await;
}

#[tokio::test]
async fn a_stranger_may_not_resolve_and_a_missing_fact_cannot_be_disputed() {
    let h = Harness::new("brain_disputes_stranger").await;
    let (owner, fact) = captured(&h, "acme").await;
    let stranger = Harness::caller(&h.user("stranger", &[]).await, &[]);

    let dispute = h
        .service
        .dispute(&Harness::caller(&owner, &[]), &fact.id, "mine")
        .await
        .expect("owner disputes");
    let error = h
        .service
        .resolve_dispute(
            &stranger,
            &resolution(&fact.id, &dispute, DisputeOutcome::Upheld),
        )
        .await
        .expect_err("a stranger may not govern");
    assert!(
        matches!(error, BrainError::NotFound(ref id) if id == &fact.id),
        "a fact the stranger cannot see is not found: {error:?}"
    );
    assert_eq!(open_disputes(&h, &fact.id).await, 1);

    let error = h
        .service
        .dispute(&stranger, &BrainFactId::generate(), "missing")
        .await
        .expect_err("missing fact");
    assert!(matches!(error, BrainError::NotFound(_)), "{error:?}");
    h.finish().await;
}

#[tokio::test]
async fn an_entered_fact_of_an_undeclared_kind_is_refused_and_a_declared_one_lands() {
    let h = Harness::new("brain_disputes_enter").await;
    let admin = h.user("admin", &["admin"]).await;
    let caller = Harness::caller(&admin, &["admin"]);
    let mut fact = EnteredFact {
        kind: kind("undeclared"),
        subject: SubjectKey::new("product", "acme"),
        fields: serde_json::Map::from_iter([("team".to_owned(), serde_json::json!("ops"))]),
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Candidate,
        observed_at: None,
    };
    let error = h
        .service
        .enter(&caller, &fact)
        .await
        .expect_err("undeclared kind");
    assert!(
        matches!(error, BrainError::UnknownKind(ref k) if k.as_str() == "undeclared"),
        "{error:?}"
    );
    assert_eq!(h.total("event_outbox").await, 0);

    fact.kind = kind(OWNERSHIP);
    h.service.enter(&caller, &fact).await.expect("enter");
    h.drain().await;
    let landed = h.only_fact("acme").await;
    assert_eq!(landed.kind, kind(OWNERSHIP));
    assert_eq!(landed.fields.get("team"), Some(&serde_json::json!("ops")));
    h.finish().await;
}
