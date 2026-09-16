//! Snapshot ranges, processing health and durable SSE generation contracts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::builder::Document;
use crate::routes::managed::snapshot_stream::FeedbackWake;
use crate::routes::managed::snapshots as api;
use systemprompt_analytics::snapshots::{
    FeedbackSnapshot, SnapshotHealth, SnapshotRangeJob, SnapshotRangeRequest,
};
pub(super) fn register(d: &mut Document) {
    d.add::<(), api::Page>("/analytics/snapshots", "get", 200, false);
    d.query::<api::Cursor>("/analytics/snapshots", "get");
    for path in [
        "/analytics/snapshots/portfolio",
        "/analytics/snapshots/{resource}",
    ] {
        d.add::<(), Option<FeedbackSnapshot>>(path, "get", 200, false);
        d.query::<api::Window>(path, "get");
    }
    d.add::<(), SnapshotHealth>("/analytics/status", "get", 200, false);
    d.add::<SnapshotRangeRequest, SnapshotRangeJob>("/analytics/jobs", "post", 202, false);
    d.add::<(), SnapshotRangeJob>("/analytics/jobs/{operation}", "get", 200, false);
    d.stream::<FeedbackWake>("/analytics/live");
    d.query::<crate::routes::managed::snapshot_stream::Resume>("/analytics/live", "get");
}
