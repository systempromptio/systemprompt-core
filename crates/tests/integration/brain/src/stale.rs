use systemprompt_identifiers::DocumentCommentId;
use systemprompt_models::brain::FactStatus;

use crate::events::{Landed, comment_resolved, price};
use crate::harness::Harness;

#[tokio::test]
async fn a_new_head_revision_stales_the_facts_of_the_previous_one() {
    let h = Harness::new("brain_stale_revision").await;
    let author = h.user("author", &[]).await;
    let first = Landed::new();
    let other = Landed::new();
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.model
        .push(serde_json::json!([price("gizmo", "emea", "80")]));
    h.emit(&first.event(&author)).await;
    h.emit(&other.event(&author)).await;
    h.drain().await;

    h.emit(&first.next_revision().event(&author)).await;
    h.drain().await;

    assert_eq!(h.only_fact("acme").await.status, FactStatus::Stale);
    assert_eq!(
        h.only_fact("gizmo").await.status,
        FactStatus::Proposed,
        "negative control: another document's facts stay live"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_resolved_comment_stales_the_facts_sourced_from_it() {
    let h = Harness::new("brain_stale_comment").await;
    let author = h.user("author", &[]).await;
    let doc = Landed::new();
    let comment = DocumentCommentId::generate();
    let other = DocumentCommentId::generate();
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.model
        .push(serde_json::json!([price("gizmo", "emea", "80")]));
    h.emit(&comment_resolved(&author, &doc.document, &comment))
        .await;
    h.emit(&comment_resolved(&author, &doc.document, &other))
        .await;
    h.drain().await;
    assert_eq!(h.only_fact("acme").await.status, FactStatus::Proposed);

    h.emit(&comment_resolved(&author, &doc.document, &comment))
        .await;
    h.drain().await;

    assert_eq!(h.only_fact("acme").await.status, FactStatus::Stale);
    assert_eq!(
        h.only_fact("gizmo").await.status,
        FactStatus::Proposed,
        "negative control: another comment's facts stay live"
    );
    h.finish().await;
}
