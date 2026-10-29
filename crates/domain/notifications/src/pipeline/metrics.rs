//! The notifications pipeline's Prometheus series.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) fn fact(outcome: &'static str) {
    metrics::counter!("notifications_facts_total", "outcome" => outcome).increment(1);
}

pub(crate) fn recipients(decision: &'static str, count: usize) {
    if count > 0 {
        metrics::counter!("notifications_recipients_total", "decision" => decision)
            .increment(u64::try_from(count).unwrap_or(u64::MAX));
    }
}

pub(crate) fn delivery(surface: &'static str, status: &'static str) {
    metrics::counter!(
        "notifications_deliveries_total",
        "surface" => surface,
        "status" => status,
    )
    .increment(1);
}

pub(crate) fn batch_seconds(seconds: f64) {
    metrics::histogram!("notifications_batch_seconds").record(seconds);
}

pub(crate) fn backlog(pending: i64, oldest_seconds: Option<i64>) {
    metrics::gauge!("notifications_backlog_pending").set(pending as f64);
    metrics::gauge!("notifications_backlog_oldest_seconds").set(oldest_seconds.unwrap_or(0) as f64);
}
