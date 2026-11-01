use crate::events::{Landed, price};
use crate::harness::{Harness, Setup};

#[tokio::test]
async fn with_no_filter_registered_the_event_records_no_filter_and_no_facts() {
    let h = Harness::with(
        "brain_no_filter",
        Setup {
            gather: None,
            ..Setup::default()
        },
    )
    .await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));

    let id = h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1 \
             AND outcome = 'no_filter' AND filter IS NULL",
            id.as_str()
        )
        .await,
        1
    );
    assert_eq!(h.total("brain_facts").await, 0);
    assert_eq!(h.model.calls(), 0, "nothing was extracted");
    h.finish().await;
}

#[tokio::test]
async fn negative_control_a_gathering_filter_does_extract() {
    let h = Harness::new("brain_no_filter_control").await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));

    let id = h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_event_results WHERE outbox_id = $1 \
             AND outcome = 'processed' AND filter = 'test.gather'",
            id.as_str()
        )
        .await,
        1
    );
    assert_eq!(h.total("brain_facts").await, 1);
    h.finish().await;
}
