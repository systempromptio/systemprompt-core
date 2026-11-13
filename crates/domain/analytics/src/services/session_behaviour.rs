//! Behavioral bot detection input collection over fingerprint history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use std::sync::Arc;

use systemprompt_analytics::{
    BehavioralAnalysisInput, BehavioralBotDetector, SessionSignalsRepository,
};
use systemprompt_identifiers::SessionId;
use systemprompt_traits::{BackgroundTasks, DynSessionStore, SessionStore};

const BEHAVIORAL_FINGERPRINT_WINDOW_DAYS: i64 = 45;

#[derive(Debug, Clone)]
pub struct DetectionSubject {
    pub session_id: SessionId,
    pub fingerprint_hash: Option<String>,
    pub user_agent: Option<String>,
    pub request_count: i64,
}

pub(super) fn spawn_behavioral_detection_task(
    background: &BackgroundTasks,
    sessions: DynSessionStore,
    signals: Arc<SessionSignalsRepository>,
    subject: DetectionSubject,
) {
    background.spawn("analytics_behavioral_detection", async move {
        let session_id = subject.session_id.clone();
        let input = collect_analysis_input(&*sessions, &signals, subject).await;

        let result = BehavioralBotDetector::analyze(&input);

        if result.score > 0
            && let Err(e) = sessions
                .update_behavioral_detection(
                    &session_id,
                    result.score,
                    result.is_suspicious,
                    result.reason.as_deref(),
                )
                .await
        {
            tracing::error!(error = %e, "Failed to update behavioral detection");
        }
    });
}

pub async fn collect_analysis_input(
    sessions: &dyn SessionStore,
    signals: &SessionSignalsRepository,
    subject: DetectionSubject,
) -> BehavioralAnalysisInput {
    let DetectionSubject {
        session_id,
        fingerprint_hash,
        user_agent,
        request_count,
    } = subject;
    let fingerprint = fingerprint_stats(sessions, signals, fingerprint_hash.as_deref()).await;

    let endpoints_accessed = signals
        .get_endpoint_sequence(&session_id)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to get endpoint sequence");
            Vec::new()
        });

    let request_timestamps = signals
        .get_request_timestamps(&session_id)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to get request timestamps");
            Vec::new()
        });

    let total_site_pages = signals.get_total_content_pages().await.unwrap_or_else(|e| {
        tracing::debug!(error = %e, "Failed to get total content pages");
        100
    });

    let has_javascript_events = signals
        .has_analytics_events(&session_id)
        .await
        .unwrap_or(false);

    let timeline = session_timeline(sessions, &session_id, request_count).await;

    BehavioralAnalysisInput {
        session_id,
        fingerprint_hash,
        user_agent,
        request_count: timeline.request_count,
        started_at: timeline.started_at,
        last_activity_at: timeline.last_activity_at,
        endpoints_accessed,
        total_site_pages,
        fingerprint_session_count: fingerprint.session_count,
        fingerprint_unique_ip_count: fingerprint.unique_ip_count,
        fingerprint_engagement_event_count: fingerprint.engagement_event_count,
        fingerprint_session_starts: fingerprint.session_starts,
        request_timestamps,
        has_javascript_events,
        landing_page: timeline.landing_page,
        entry_url: timeline.entry_url,
    }
}

struct FingerprintStats {
    session_count: i64,
    unique_ip_count: i64,
    engagement_event_count: i64,
    session_starts: Vec<DateTime<Utc>>,
}

async fn fingerprint_stats(
    sessions: &dyn SessionStore,
    signals: &SessionSignalsRepository,
    fingerprint: Option<&str>,
) -> FingerprintStats {
    let Some(fp) = fingerprint else {
        return FingerprintStats {
            session_count: 1,
            unique_ip_count: 0,
            engagement_event_count: 0,
            session_starts: Vec::new(),
        };
    };

    let session_count = sessions
        .count_sessions_by_fingerprint(fp, 24)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to count fingerprint sessions");
            1
        });

    let unique_ip_count = sessions
        .count_unique_ips_by_fingerprint(fp, BEHAVIORAL_FINGERPRINT_WINDOW_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to count fingerprint unique IPs");
            0
        });

    let engagement_event_count = signals
        .count_engagement_events_by_fingerprint(fp, BEHAVIORAL_FINGERPRINT_WINDOW_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to count fingerprint engagement events");
            0
        });

    let session_starts = sessions
        .get_session_starts_by_fingerprint(fp, BEHAVIORAL_FINGERPRINT_WINDOW_DAYS)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to load fingerprint session starts");
            Vec::new()
        });

    FingerprintStats {
        session_count,
        unique_ip_count,
        engagement_event_count,
        session_starts,
    }
}

struct SessionTimeline {
    started_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
    request_count: i64,
    landing_page: Option<String>,
    entry_url: Option<String>,
}

async fn session_timeline(
    sessions: &dyn SessionStore,
    session_id: &SessionId,
    request_count: i64,
) -> SessionTimeline {
    let session_data = sessions
        .get_session_for_behavioral_analysis(session_id)
        .await
        .map_err(|e| {
            tracing::debug!(error = %e, "Failed to get session for behavioral analysis");
            e
        })
        .ok()
        .flatten();

    session_data.map_or_else(
        || {
            let now = Utc::now();
            SessionTimeline {
                started_at: now,
                last_activity_at: now,
                request_count,
                landing_page: None,
                entry_url: None,
            }
        },
        |s| SessionTimeline {
            started_at: s.started_at,
            last_activity_at: s.last_activity_at,
            request_count: s.request_count.map_or(request_count, i64::from),
            landing_page: s.landing_page,
            entry_url: s.entry_url,
        },
    )
}
