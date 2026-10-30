use systemprompt_models::brain::FactStatus;

use crate::events::{Landed, price};
use crate::harness::Harness;

const DISPUTES: &str = "SELECT COUNT(*) FROM brain_disputes WHERE fact_id = $1 AND status = 'open'";

#[tokio::test]
async fn a_near_identical_contradiction_opens_a_dispute_and_never_overwrites() {
    let h = Harness::new("brain_dedupe_dispute").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let original = h.only_fact("acme").await;

    h.model
        .push(serde_json::json!([price("acme", "apac", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    let after = h.only_fact("acme").await;
    assert_eq!(after.id, original.id, "no second fact was inserted");
    assert_eq!(
        after.statement, original.statement,
        "the statement is untouched"
    );
    assert_eq!(after.fields, original.fields, "the fields are untouched");
    assert_eq!(after.status, FactStatus::Disputed);
    assert_eq!(h.count(DISPUTES, original.id.as_str()).await, 1);
    h.finish().await;
}

#[tokio::test]
async fn the_same_identity_revises_the_fact_instead_of_adding_one() {
    let h = Harness::new("brain_dedupe_revise").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let original = h.only_fact("acme").await;

    h.model
        .push(serde_json::json!([price("acme", "emea", "1350")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    let revised = h.only_fact("acme").await;
    assert_eq!(revised.id, original.id);
    assert_eq!(revised.revision, 2);
    assert!(revised.statement.contains("1350"), "{}", revised.statement);
    assert_eq!(h.count(DISPUTES, original.id.as_str()).await, 0);
    h.finish().await;
}

#[tokio::test]
async fn an_erased_identity_is_rejected_on_re_extraction() {
    let h = Harness::new("brain_dedupe_tombstone").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let fact = h.only_fact("acme").await;
    h.service
        .erase(&Harness::caller(&author, &[]), &fact.id)
        .await
        .expect("owner erases");

    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    let again = h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    assert!(h.fact_ids("acme").await.is_empty(), "the tombstone held");
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1 AND outcome = 'rejected'",
            again.as_str()
        )
        .await,
        1
    );

    h.model
        .push(serde_json::json!([price("acme", "apac", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    assert_eq!(
        h.fact_ids("acme").await.len(),
        1,
        "negative control: a different identity is still captured"
    );
    h.finish().await;
}
