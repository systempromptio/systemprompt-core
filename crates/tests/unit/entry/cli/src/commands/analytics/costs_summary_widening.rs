//! Tests for the `analytics costs summary` window resolution: the default
//! 24h window widens through 7d then 30d while it holds no requests, and an
//! explicit `--since`/`--until` pins the window.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::cell::RefCell;

use chrono::{DateTime, Duration, Utc};
use systemprompt_analytics::models::reporting::CostSummaryRow;
use systemprompt_cli::analytics::costs::summary::resolve_window;

fn assert_spans(actual: Duration, expected: Duration) {
    let drift = (actual - expected).num_milliseconds().abs();
    assert!(
        drift < 1000,
        "expected a {expected:?} window, got {actual:?}"
    );
}

fn row(requests: i64) -> CostSummaryRow {
    CostSummaryRow {
        requests,
        cost: Some(requests * 100),
        tokens: Some(requests * 10),
    }
}

struct FetchLog {
    windows: RefCell<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
    responses: RefCell<Vec<CostSummaryRow>>,
}

impl FetchLog {
    fn new(responses: Vec<CostSummaryRow>) -> Self {
        Self {
            windows: RefCell::new(Vec::new()),
            responses: RefCell::new(responses),
        }
    }

    fn fetch(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> CostSummaryRow {
        self.windows.borrow_mut().push((start, end));
        self.responses.borrow_mut().remove(0)
    }
}

#[tokio::test]
async fn default_window_with_requests_does_not_widen() {
    let log = FetchLog::new(vec![row(3)]);
    let resolved = resolve_window(None, None, |s, e| {
        let r = log.fetch(s, e);
        async move { Ok(r) }
    })
    .await
    .unwrap();

    assert_eq!(resolved.summary.requests, 3);
    assert_eq!(resolved.widened_to, None);
    assert_eq!(log.windows.borrow().len(), 1);
    let (start, end) = log.windows.borrow()[0];
    assert_spans(end - start, Duration::hours(24));
}

#[tokio::test]
async fn empty_default_window_widens_to_seven_days() {
    let log = FetchLog::new(vec![row(0), row(5)]);
    let resolved = resolve_window(None, None, |s, e| {
        let r = log.fetch(s, e);
        async move { Ok(r) }
    })
    .await
    .unwrap();

    assert_eq!(resolved.widened_to, Some("7d"));
    assert_eq!(resolved.summary.requests, 5);
    assert_spans(resolved.end - resolved.start, Duration::days(7));
    assert_eq!(log.windows.borrow().len(), 2);
}

#[tokio::test]
async fn empty_seven_day_window_widens_to_thirty_days() {
    let log = FetchLog::new(vec![row(0), row(0), row(2)]);
    let resolved = resolve_window(None, None, |s, e| {
        let r = log.fetch(s, e);
        async move { Ok(r) }
    })
    .await
    .unwrap();

    assert_eq!(resolved.widened_to, Some("30d"));
    assert_eq!(resolved.summary.requests, 2);
    assert_spans(resolved.end - resolved.start, Duration::days(30));
    assert_eq!(log.windows.borrow().len(), 3);
}

#[tokio::test]
async fn empty_everywhere_falls_back_to_the_default_window() {
    let log = FetchLog::new(vec![row(0), row(0), row(0)]);
    let resolved = resolve_window(None, None, |s, e| {
        let r = log.fetch(s, e);
        async move { Ok(r) }
    })
    .await
    .unwrap();

    assert_eq!(resolved.widened_to, None);
    assert_eq!(resolved.summary.requests, 0);
    assert_spans(resolved.end - resolved.start, Duration::hours(24));
    assert_eq!(log.windows.borrow().len(), 3);
}

#[tokio::test]
async fn explicit_since_never_widens() {
    let since = "1h".to_string();
    let log = FetchLog::new(vec![row(0)]);
    let resolved = resolve_window(Some(&since), None, |s, e| {
        let r = log.fetch(s, e);
        async move { Ok(r) }
    })
    .await
    .unwrap();

    assert_eq!(resolved.widened_to, None);
    assert_eq!(resolved.summary.requests, 0);
    assert_eq!(log.windows.borrow().len(), 1);
    let (start, end) = log.windows.borrow()[0];
    assert_spans(end - start, Duration::hours(1));
}

#[tokio::test]
async fn explicit_until_never_widens() {
    let until = "24h".to_string();
    let log = FetchLog::new(vec![row(0)]);
    let resolved = resolve_window(None, Some(&until), |s, e| {
        let r = log.fetch(s, e);
        async move { Ok(r) }
    })
    .await
    .unwrap();

    assert_eq!(resolved.widened_to, None);
    assert_eq!(log.windows.borrow().len(), 1);
}
