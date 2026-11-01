use chrono::{Duration, Utc};
use systemprompt_brain::BrainError;
use systemprompt_brain::service::{DisputeOutcome, DisputeResolution};
use systemprompt_models::brain::FactStatus;

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::plane::PRICING_REVIEW_DAYS;
use crate::recall_support::{ids, recall};

async fn captured(
    h: &Harness,
    product: &str,
) -> (
    systemprompt_identifiers::UserId,
    systemprompt_models::brain::BrainFact,
) {
    let owner = h.user("owner", &[]).await;
    h.model
        .push(serde_json::json!([price(product, "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let fact = h.only_fact(product).await;
    (owner, fact)
}

#[tokio::test]
async fn the_scope_owner_confirms_and_the_review_clock_restarts_a_stranger_is_refused() {
    let h = Harness::new("brain_lifecycle_confirm").await;
    let (owner, fact) = captured(&h, "acme").await;
    let stranger = h.user("stranger", &[]).await;
    sqlx::query("UPDATE brain_facts SET review_after = now() - interval '1 day' WHERE id = $1")
        .bind(fact.id.as_str())
        .execute(&h.pg)
        .await
        .expect("age the review clock");

    let refused = h
        .service
        .confirm(&Harness::caller(&stranger, &[]), &fact.id)
        .await
        .expect_err("a stranger may not confirm a personal fact");
    assert!(
        matches!(refused, BrainError::Forbidden { .. }),
        "{refused:?}"
    );
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Proposed);

    h.service
        .confirm(&Harness::caller(&owner, &[]), &fact.id)
        .await
        .expect("the owner confirms");
    let confirmed = h.fact(&fact.id).await;
    assert_eq!(confirmed.status, FactStatus::Confirmed);
    assert_eq!(confirmed.confirmed_by.as_ref(), Some(&owner));
    let due = confirmed.review_after.expect("review_after reset");
    let expected = Utc::now() + Duration::days(i64::from(PRICING_REVIEW_DAYS));
    assert!(
        (due - expected).num_seconds().abs() < 120,
        "{due} vs {expected}"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_retracted_fact_is_retired_kept_and_never_recalled() {
    let h = Harness::new("brain_lifecycle_retract").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    h.service.confirm(&caller, &fact.id).await.expect("confirm");
    assert_eq!(
        ids(&recall(&h, &owner, "what does acme cost").await),
        vec![fact.id.clone()],
        "negative control: the confirmed fact is recalled"
    );

    h.service
        .retract(&caller, &fact.id, "superseded by the new price list")
        .await
        .expect("retract");

    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Retired);
    assert!(ids(&recall(&h, &owner, "what does acme cost").await).is_empty());
    h.finish().await;
}

#[tokio::test]
async fn an_erased_fact_leaves_a_tombstone_and_its_identity_is_refused_again() {
    let h = Harness::new("brain_lifecycle_erase").await;
    let (owner, fact) = captured(&h, "acme").await;
    let stranger = h.user("stranger", &[]).await;
    assert!(
        h.service
            .erase(&Harness::caller(&stranger, &[]), &fact.id)
            .await
            .is_err(),
        "a stranger may not erase"
    );

    h.service
        .erase(&Harness::caller(&owner, &[]), &fact.id)
        .await
        .expect("erase");

    assert!(h.fact_ids("acme").await.is_empty());
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_tombstones WHERE identity_hash = $1",
            &fact.identity_hash
        )
        .await,
        1
    );
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    assert!(h.fact_ids("acme").await.is_empty(), "re-extraction refused");
    h.finish().await;
}

#[tokio::test]
async fn an_upheld_dispute_retires_the_fact() {
    let h = Harness::new("brain_lifecycle_dispute").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    let dispute = h
        .service
        .dispute(&caller, &fact.id, "the price list moved")
        .await
        .expect("dispute");
    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Disputed);

    h.service
        .resolve_dispute(
            &caller,
            &DisputeResolution {
                fact_id: fact.id.clone(),
                dispute_id: dispute,
                outcome: DisputeOutcome::Upheld,
                note: Some("confirmed wrong".to_owned()),
            },
        )
        .await
        .expect("resolve");

    assert_eq!(h.fact(&fact.id).await.status, FactStatus::Retired);
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_disputes WHERE fact_id = $1 AND status = 'resolved'",
            fact.id.as_str()
        )
        .await,
        1
    );
    h.finish().await;
}
