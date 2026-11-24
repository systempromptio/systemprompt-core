//! The background tasks a tracked request offers to the analytics queue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::AnalyticsQueue;
use systemprompt_identifiers::SessionId;
use systemprompt_security::ScannerDetector;
use systemprompt_traits::{
    DetectionSubject, DynContentCatalogStats, DynSessionBehaviourScorer, DynSessionTelemetry,
};

pub(super) fn spawn_session_tracking_task(
    queue: &AnalyticsQueue,
    sessions: DynSessionTelemetry,
    session_id: SessionId,
) {
    // Why: one UPDATE per request — the increment also stamps
    // last_activity_at and duration_seconds.
    queue.offer(async move {
        if let Err(e) = sessions.increment_request_count(&session_id).await {
            tracing::error!(error = %e, "Failed to record session request");
        }
    });
}

pub(super) fn spawn_velocity_scanner_check(
    queue: &AnalyticsQueue,
    sessions: DynSessionTelemetry,
    session_id: SessionId,
) {
    queue.offer(async move {
        let (request_count, duration_seconds) =
            match sessions.get_session_velocity(&session_id).await {
                Ok(velocity) => velocity,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        session_id = %session_id,
                        "Failed to read session velocity; scanner check skipped"
                    );
                    return;
                },
            };

        if let (Some(count), Some(duration)) = (request_count, duration_seconds)
            && ScannerDetector::is_high_velocity(count, duration)
            && let Err(e) = sessions.mark_as_scanner(&session_id).await
        {
            tracing::warn!(
                error = %e,
                session_id = %session_id,
                "Failed to mark high-velocity session as scanner"
            );
        }
    });
}

pub(super) fn spawn_mark_scanner_task(
    queue: &AnalyticsQueue,
    sessions: DynSessionTelemetry,
    session_id: SessionId,
) {
    queue.offer(async move {
        if let Err(e) = sessions.mark_as_scanner(&session_id).await {
            tracing::warn!(error = %e, session_id = %session_id, "Failed to mark session as scanner");
        }
    });
}

pub(super) fn spawn_behavioral_detection_task(
    queue: &AnalyticsQueue,
    scorer: DynSessionBehaviourScorer,
    content: DynContentCatalogStats,
    subject: DetectionSubject,
) {
    queue.offer(async move {
        if let Err(e) = scorer.score(subject, &*content).await {
            tracing::error!(error = %e, "Failed to update behavioral detection");
        }
    });
}
