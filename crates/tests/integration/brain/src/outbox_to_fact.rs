use systemprompt_brain::pipeline::ProcessOutcome;
use systemprompt_models::brain::FactStatus;

use crate::events::{Landed, price};
use crate::harness::Harness;

const BRAIN_OUTBOX: &str =
    "SELECT COUNT(*) FROM event_outbox WHERE consumer = 'brain' AND actor_id = $1";

#[tokio::test]
async fn a_landed_revision_becomes_one_fact_with_its_first_revision_and_a_result_row() {
    let h = Harness::new("brain_outbox_fact").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));

    let id = h.emit(&Landed::new().event(&author)).await;
    let drained = h.drain().await;

    assert_eq!(drained.len(), 1, "the one pending event is drained");
    let fact = h.only_fact("acme").await;
    assert_eq!(fact.revision, 1);
    assert_eq!(fact.status, FactStatus::Proposed);
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_fact_revisions WHERE fact_id = $1 AND revision = 1",
            fact.id.as_str()
        )
        .await,
        1
    );
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1 \
             AND outcome = 'processed' AND facts_written = 1",
            id.as_str()
        )
        .await,
        1
    );
    assert!(
        h.drain().await.is_empty(),
        "the outbox row was acknowledged"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_rolled_back_producer_transaction_leaves_no_outbox_row() {
    let h = Harness::new("brain_outbox_rollback").await;
    let author = h.user("author", &[]).await;

    let mut tx = h.pg.begin().await.expect("begin");
    h.sink
        .emit(&mut tx, &Landed::new().event(&author))
        .await
        .expect("emit");
    tx.rollback().await.expect("rollback");
    assert_eq!(h.count(BRAIN_OUTBOX, author.as_str()).await, 0);
    assert!(h.drain().await.is_empty());
    assert_eq!(h.total("brain_facts").await, 0);

    h.emit(&Landed::new().event(&author)).await;
    assert_eq!(
        h.count(BRAIN_OUTBOX, author.as_str()).await,
        1,
        "negative control: a committed emit does append"
    );
    h.finish().await;
}

#[tokio::test]
async fn redelivery_under_the_same_pipeline_version_is_a_no_op() {
    let h = Harness::new("brain_outbox_redeliver").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.model
        .push(serde_json::json!([price("acme", "emea", "9999")]));
    let event = Landed::new().event(&author);
    let id = h.emit(&event).await;
    h.drain().await;

    let again = h.reprocess(&id, event).await;

    assert_eq!(again, ProcessOutcome::Skipped);
    assert_eq!(h.model.calls(), 1, "the model is not asked twice");
    let fact = h.only_fact("acme").await;
    assert_eq!(fact.revision, 1, "exactly one revision");
    assert!(fact.statement.contains("1200"));
    h.finish().await;
}
