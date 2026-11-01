use chrono::{Duration, Utc};

use crate::events::{Landed, price};
use crate::harness::{Harness, Setup};
use crate::plane::{OWNERSHIP, PRICING, PRICING_REVIEW_DAYS};

fn assert_near(actual: Option<chrono::DateTime<Utc>>, days: u32, what: &str) {
    let expected = Utc::now() + Duration::days(i64::from(days));
    let actual = actual.unwrap_or_else(|| panic!("{what}: review_after is set at insert"));
    let drift = (actual - expected).num_seconds().abs();
    assert!(
        drift < 120,
        "{what}: review_after {actual} is not now + {days} days ({expected})"
    );
}

#[tokio::test]
async fn a_pipeline_inserted_fact_carries_review_after_from_its_kind_or_the_profile() {
    let h = Harness::with(
        "brain_review_after",
        Setup {
            gather: Some(vec![PRICING, OWNERSHIP]),
            ..Setup::default()
        },
    )
    .await;
    let author = h.user("author", &[]).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.parser.emit("gizmo", "platform");

    h.emit(&Landed::new().event(&author)).await;
    h.drain().await;

    let priced = h.only_fact("acme").await;
    assert_near(priced.review_after, PRICING_REVIEW_DAYS, "kind days");
    let owned = h.only_fact("gizmo").await;
    assert_near(
        owned.review_after,
        h.profile.lifecycle.review_after_days,
        "profile default",
    );
    assert_ne!(
        PRICING_REVIEW_DAYS, h.profile.lifecycle.review_after_days,
        "negative control: the two sources of the interval differ"
    );
    h.finish().await;
}
