use chrono::Utc;
use std::sync::Arc;

use systemprompt_analytics::{
    BehavioralAnalysisInput, BehavioralBotDetector, SessionRepository, ThrottleLevel,
};
use systemprompt_identifiers::SessionId;

pub fn spawn_behavioral_detection_task(
    session_repo: Arc<SessionRepository>,
    session_id: SessionId,
    fingerprint_hash: Option<String>,
    user_agent: Option<String>,
    request_count: i64,
) {
    tokio::spawn(async move {
        let fingerprint_session_count = if let Some(ref fp) = fingerprint_hash {
            session_repo
                .count_sessions_by_fingerprint(fp, 24)
                .await
                .unwrap_or_else(|e| {
                    tracing::debug!(error = %e, "Failed to count fingerprint sessions");
                    1
                })
        } else {
            1
        };

        let endpoints_accessed = session_repo
            .get_endpoint_sequence(&session_id)
            .await
            .unwrap_or_else(|e| {
                tracing::debug!(error = %e, "Failed to get endpoint sequence");
                Vec::new()
            });

        let request_timestamps = session_repo
            .get_request_timestamps(&session_id)
            .await
            .unwrap_or_else(|e| {
                tracing::debug!(error = %e, "Failed to get request timestamps");
                Vec::new()
            });

        let total_site_pages = session_repo
            .get_total_content_pages()
            .await
            .unwrap_or_else(|e| {
                tracing::debug!(error = %e, "Failed to get total content pages");
                100
            });

        let session_data = session_repo
            .get_session_for_behavioral_analysis(&session_id)
            .await
            .map_err(|e| {
                tracing::debug!(error = %e, "Failed to get session for behavioral analysis");
                e
            })
            .ok()
            .flatten();

        let (started_at, last_activity_at) = session_data
            .map(|s| (s.started_at, s.last_activity_at))
            .unwrap_or_else(|| {
                let now = Utc::now();
                (now, now)
            });

        let session_id_for_update = session_id.clone();
        let input = BehavioralAnalysisInput {
            session_id,
            fingerprint_hash,
            user_agent,
            request_count,
            started_at,
            last_activity_at,
            endpoints_accessed,
            total_site_pages,
            fingerprint_session_count,
            request_timestamps,
        };

        let result = BehavioralBotDetector::analyze(&input);

        if result.score > 0 {
            if let Err(e) = session_repo
                .update_behavioral_detection(
                    &session_id_for_update,
                    result.score,
                    result.is_suspicious,
                    result.reason.as_deref(),
                )
                .await
            {
                tracing::error!(error = %e, "Failed to update behavioral detection");
            }

            if result.is_suspicious {
                escalate_throttle_if_needed(&session_repo, &session_id_for_update, result.score)
                    .await;
            }
        }
    });
}

async fn escalate_throttle_if_needed(
    session_repo: &SessionRepository,
    session_id: &SessionId,
    score: i32,
) {
    let current_level = session_repo
        .get_throttle_level(session_id)
        .await
        .map_err(|e| {
            tracing::debug!(error = %e, "Failed to get throttle level");
            e
        })
        .unwrap_or(0);

    let level = ThrottleLevel::from(current_level);
    let new_level = level.escalate();

    if new_level != level {
        if let Err(e) = session_repo
            .escalate_throttle(session_id, i32::from(new_level))
            .await
        {
            tracing::error!(error = %e, "Failed to escalate throttle level");
        } else {
            tracing::warn!(
                session_id = %session_id,
                score = score,
                old_level = ?level,
                new_level = ?new_level,
                "Escalated throttle level for behavioral bot"
            );
        }
    }
}
