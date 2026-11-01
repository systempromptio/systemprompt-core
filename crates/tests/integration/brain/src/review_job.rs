use std::sync::Arc;

use systemprompt_brain::jobs::BrainReviewJob;

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::jobs_support::{RecordedNotices, exec, run};

const NOTIFIED: &str =
    "SELECT COUNT(*) FROM brain_facts WHERE id = $1 AND review_notified_at IS NOT NULL";

#[tokio::test]
async fn due_proposed_and_confirmed_facts_are_marked_and_their_people_notified() {
    let h = Harness::new("brain_review_job").await;
    let author = h.user("author", &[]).await;
    for product in ["dueproposed", "dueconfirmed", "notdue"] {
        h.model
            .push(serde_json::json!([price(product, "emea", "1200")]));
        h.emit(&Landed::new().event(&author)).await;
    }
    h.drain().await;
    let proposed = h.only_fact("dueproposed").await;
    let confirmed = h.only_fact("dueconfirmed").await;
    let later = h.only_fact("notdue").await;
    h.service
        .confirm(&Harness::caller(&author, &[]), &confirmed.id)
        .await
        .expect("confirm");
    for id in [&proposed.id, &confirmed.id] {
        exec(
            &h,
            "UPDATE brain_facts SET review_after = now() - interval '1 hour' WHERE id = $1",
            id.as_str(),
        )
        .await;
    }
    let notices = Arc::new(RecordedNotices::default());

    let result = run(
        &h,
        &BrainReviewJob,
        h.profile.clone(),
        Some(Arc::clone(&notices)),
    )
    .await;
    assert!(result.success, "{result:?}");

    assert_eq!(h.count(NOTIFIED, proposed.id.as_str()).await, 1);
    assert_eq!(h.count(NOTIFIED, confirmed.id.as_str()).await, 1);
    assert_eq!(
        h.count(NOTIFIED, later.id.as_str()).await,
        0,
        "negative control: a fact not yet due is left alone"
    );
    assert_eq!(
        notices.kinds(),
        vec!["brain.fact.review_due".to_owned(); 2],
        "one review notice per due fact"
    );
    h.finish().await;
}
