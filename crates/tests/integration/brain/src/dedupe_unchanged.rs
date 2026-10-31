use crate::events::{Landed, price};
use crate::harness::Harness;

#[tokio::test]
async fn re_extracting_the_same_statement_leaves_the_fact_unchanged() {
    let h = Harness::new("brain_dedupe_unchanged").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;
    let original = h.only_fact("acme").await;

    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    let after = h.only_fact("acme").await;
    assert_eq!(after.id, original.id);
    assert_eq!(after.revision, original.revision);
    assert_eq!(after.statement, original.statement);
    assert_eq!(after.status, original.status);
    h.finish().await;
}
