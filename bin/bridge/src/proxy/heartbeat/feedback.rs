//! Feedback outbox flushing on each heartbeat tick, with change-only warnings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::proxy::session::SessionContext;
use crate::warn_once::WarnOnce;

pub(super) async fn flush_feedback(gateway: &str, session: Arc<SessionContext>, faults: &WarnOnce) {
    let gateway_for_flush = gateway.to_owned();
    let flushed =
        tokio::task::spawn_blocking(move || session.native_sessions().flush(&gateway_for_flush))
            .await
            .map_err(|error| crate::feedback::FeedbackError::Io(std::io::Error::other(error)))
            .and_then(|result| result);
    report_feedback(faults, flushed, "Native session bindings remain unqueued");
    let retried = crate::feedback::retry_pending(gateway).await;
    report_feedback(
        faults,
        retried,
        "Installation feedback retry remains pending",
    );
}

// Why: the heartbeat ticks every few seconds and an unchanged feedback fault
// warned on each one; it now warns when the error changes and stays quiet
// while it persists.
fn report_feedback(faults: &WarnOnce, result: crate::feedback::Result<()>, message: &str) {
    let current = match result {
        Ok(()) | Err(crate::feedback::FeedbackError::EnrollmentRequired) => None,
        Err(error) => Some(error.to_string()),
    };
    match current {
        None => faults.clear(message),
        Some(error) if faults.changed(message, &error) => {
            tracing::warn!(%error, stage = message, "feedback delivery remains pending");
        },
        Some(error) => {
            tracing::debug!(%error, stage = message, "feedback delivery still pending (unchanged)");
        },
    }
}
