use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use systemprompt_extension::{
    EffectiveCutoff, RetentionHold, RetentionHoldError, RetentionHolds, RetentionTarget,
};

pub(crate) struct FixedHold {
    pub(crate) name: &'static str,
    pub(crate) target: RetentionTarget,
    pub(crate) watermark: Option<DateTime<Utc>>,
    pub(crate) fails: bool,
}

#[async_trait]
impl RetentionHold for FixedHold {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn watermark(
        &self,
        target: RetentionTarget,
    ) -> Result<Option<DateTime<Utc>>, RetentionHoldError> {
        if target != self.target {
            return Ok(None);
        }
        if self.fails {
            return Err(RetentionHoldError::Unavailable(Box::new(
                std::io::Error::other("exporter offline"),
            )));
        }
        Ok(self.watermark)
    }
}

pub(crate) fn hold(
    name: &'static str,
    target: RetentionTarget,
    watermark: DateTime<Utc>,
) -> Arc<dyn RetentionHold> {
    Arc::new(FixedHold {
        name,
        target,
        watermark: Some(watermark),
        fails: false,
    })
}

pub(crate) fn failing(name: &'static str, target: RetentionTarget) -> Arc<dyn RetentionHold> {
    Arc::new(FixedHold {
        name,
        target,
        watermark: None,
        fails: true,
    })
}

#[tokio::test]
async fn no_hold_keeps_the_age_cutoff() {
    let cutoff = Utc::now();
    let effective = RetentionHolds::default()
        .resolve(RetentionTarget::Logs, cutoff)
        .await;
    assert!(matches!(effective, EffectiveCutoff::Age(c) if c == cutoff));
}

#[tokio::test]
async fn a_hold_on_another_target_keeps_the_age_cutoff() {
    let cutoff = Utc::now();
    let holds = RetentionHolds::new(vec![hold(
        "exporter",
        RetentionTarget::AnalyticsEvents,
        cutoff - Duration::days(3),
    )]);
    let effective = holds.resolve(RetentionTarget::Logs, cutoff).await;
    assert!(matches!(effective, EffectiveCutoff::Age(c) if c == cutoff));
}

#[tokio::test]
async fn a_watermark_before_the_cutoff_clamps_it() {
    let cutoff = Utc::now();
    let watermark = cutoff - Duration::days(3);
    let holds = RetentionHolds::new(vec![hold("exporter", RetentionTarget::Logs, watermark)]);
    let effective = holds.resolve(RetentionTarget::Logs, cutoff).await;
    assert!(matches!(
        effective,
        EffectiveCutoff::Held { cutoff: c, hold: "exporter" } if c == watermark
    ));
}

#[tokio::test]
async fn a_watermark_after_the_cutoff_does_not_extend_deletion() {
    let cutoff = Utc::now() - Duration::days(30);
    let holds = RetentionHolds::new(vec![hold("exporter", RetentionTarget::Logs, Utc::now())]);
    let effective = holds.resolve(RetentionTarget::Logs, cutoff).await;
    assert!(matches!(effective, EffectiveCutoff::Age(c) if c == cutoff));
}

#[tokio::test]
async fn the_earliest_of_two_watermarks_wins_in_either_order() {
    let cutoff = Utc::now();
    let early = cutoff - Duration::days(5);
    let late = cutoff - Duration::days(2);
    for holds in [
        vec![
            hold("late", RetentionTarget::Logs, late),
            hold("early", RetentionTarget::Logs, early),
        ],
        vec![
            hold("early", RetentionTarget::Logs, early),
            hold("late", RetentionTarget::Logs, late),
        ],
    ] {
        let effective = RetentionHolds::new(holds)
            .resolve(RetentionTarget::Logs, cutoff)
            .await;
        assert!(matches!(
            effective,
            EffectiveCutoff::Held { cutoff: c, hold: "early" } if c == early
        ));
    }
}

#[tokio::test]
async fn a_failing_hold_withholds_the_target_even_beside_a_working_one() {
    let cutoff = Utc::now();
    let holds = RetentionHolds::new(vec![
        hold(
            "exporter",
            RetentionTarget::Logs,
            cutoff - Duration::days(1),
        ),
        failing("broken", RetentionTarget::Logs),
    ]);
    let effective = holds.resolve(RetentionTarget::Logs, cutoff).await;
    assert!(effective.cutoff().is_none());
    assert!(matches!(
        effective,
        EffectiveCutoff::Withheld { hold: "broken", .. }
    ));
}

#[tokio::test]
async fn a_failing_hold_leaves_other_targets_unclamped() {
    let cutoff = Utc::now();
    let holds = RetentionHolds::new(vec![failing("broken", RetentionTarget::Logs)]);
    let effective = holds
        .resolve(RetentionTarget::AnalyticsEvents, cutoff)
        .await;
    assert!(matches!(effective, EffectiveCutoff::Age(c) if c == cutoff));
}

#[tokio::test]
async fn merged_holds_keep_both_sets() {
    let cutoff = Utc::now();
    let early = cutoff - Duration::days(4);
    let holds = RetentionHolds::new(vec![hold(
        "first",
        RetentionTarget::Logs,
        cutoff - Duration::days(1),
    )])
    .merged(&RetentionHolds::new(vec![hold(
        "second",
        RetentionTarget::Logs,
        early,
    )]));
    let effective = holds.resolve(RetentionTarget::Logs, cutoff).await;
    assert!(matches!(
        effective,
        EffectiveCutoff::Held { cutoff: c, hold: "second" } if c == early
    ));
}

#[test]
fn every_target_names_a_distinct_table() {
    let tables: HashSet<_> = RetentionTarget::ALL.iter().map(|t| t.table()).collect();
    assert_eq!(tables.len(), RetentionTarget::ALL.len());
}
