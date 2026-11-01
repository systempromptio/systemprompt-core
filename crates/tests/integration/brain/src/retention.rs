use systemprompt_brain::jobs::BrainRetentionJob;
use systemprompt_models::brain::FactStatus;

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::jobs_support::{exec, run};

async fn capture(h: &Harness, products: &[&str]) {
    let author = h.user("author", &[]).await;
    for product in products {
        h.model
            .push(serde_json::json!([price(product, "emea", "1200")]));
        h.emit(&Landed::new().event(&author)).await;
    }
    h.drain().await;
}

#[tokio::test]
async fn retention_retires_expired_facts_deletes_old_rows_and_keeps_legal_holds() {
    let h = Harness::new("brain_retention").await;
    capture(&h, &["expiring", "fresh", "oldretired", "heldretired"]).await;
    let expiring = h.only_fact("expiring").await;
    let fresh = h.only_fact("fresh").await;
    let old = h.only_fact("oldretired").await;
    let held = h.only_fact("heldretired").await;
    exec(
        &h,
        "UPDATE brain_facts SET retention_until = now() - interval '1 hour' WHERE id = $1",
        expiring.id.as_str(),
    )
    .await;
    for (id, hold) in [(&old.id, "false"), (&held.id, "true")] {
        sqlx::query(
            "UPDATE brain_facts SET status = 'retired', legal_hold = $2::boolean, \
             updated_at = now() - interval '400 days' WHERE id = $1",
        )
        .bind(id.as_str())
        .bind(hold)
        .execute(&h.pg)
        .await
        .expect("age retired fact");
    }
    exec(
        &h,
        "UPDATE brain_event_results SET claimed_at = now() - interval '40 days' \
         WHERE outbox_id = (SELECT outbox_id FROM brain_event_results ORDER BY claimed_at LIMIT 1) \
         AND $1 <> ''",
        "x",
    )
    .await;
    let results_before = h.total("brain_event_results").await;

    let result = run(&h, &BrainRetentionJob, h.profile.clone(), None).await;
    assert!(result.success, "{result:?}");

    assert_eq!(h.fact(&expiring.id).await.status, FactStatus::Retired);
    assert_eq!(
        h.fact(&fresh.id).await.status,
        FactStatus::Proposed,
        "negative control: an unexpired fact is untouched"
    );
    assert!(
        h.fact_ids("oldretired").await.is_empty(),
        "old retired fact deleted"
    );
    assert_eq!(h.fact_ids("heldretired").await.len(), 1, "legal hold kept");
    assert_eq!(h.total("brain_event_results").await, results_before - 1);
    let revisions = h
        .count(
            "SELECT COUNT(*) FROM brain_fact_revisions WHERE fact_id = $1 AND change = 'expired'",
            expiring.id.as_str(),
        )
        .await;
    assert_eq!(revisions, 1, "expiry is a revision like any change");
    h.finish().await;
}
