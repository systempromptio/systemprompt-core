//! Request analytics middleware.
//!
//! [`AnalyticsMiddleware`] records tracked requests after the response is
//! produced, running session activity, velocity-based scanner detection,
//! behavioural bot scoring, and analytics-event capture on the process's
//! [`BackgroundTasks`] so the request path is never blocked on persistence and
//! shutdown drains every pending write.
//!
//! The session store and the behavioural scorer are read from the extension
//! state (provided by the `users` and `analytics` extensions) through their
//! shared traits. Recording is not a guard: when either is absent the request
//! is served and that part of the record is skipped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod events;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

use systemprompt_extension::ExtensionContext as _;
use systemprompt_identifiers::SessionId;
use systemprompt_logging::AnalyticsRepository;
use systemprompt_models::{RequestContext, RouteClassifier};
use systemprompt_runtime::AppContext;
use systemprompt_security::ScannerDetector;
use systemprompt_traits::{
    BackgroundTasks, DetectionSubject, DynSessionBehaviourScorer, DynSessionStore,
};

pub use events::AnalyticsEventParams;

struct TrackingParams<'a> {
    req_ctx: &'a RequestContext,
    uri: &'a http::Uri,
    method: &'a http::Method,
    status_code: u16,
    response_time_ms: u64,
    user_agent: Option<String>,
    referer: Option<String>,
    is_scanner: bool,
    html_response: bool,
}

#[derive(Clone)]
pub struct AnalyticsMiddleware {
    sessions: Option<DynSessionStore>,
    scorer: Option<DynSessionBehaviourScorer>,
    analytics_repo: Arc<AnalyticsRepository>,
    route_classifier: Arc<RouteClassifier>,
    background: BackgroundTasks,
}

impl std::fmt::Debug for AnalyticsMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsMiddleware")
            .field("records_sessions", &self.sessions.is_some())
            .field("scores_behaviour", &self.scorer.is_some())
            .field("analytics_repo", &self.analytics_repo)
            .field("route_classifier", &self.route_classifier)
            .field("background", &self.background)
            .finish_non_exhaustive()
    }
}

impl AnalyticsMiddleware {
    pub fn new(app_context: &AppContext) -> Self {
        let state = app_context.extension_state();
        let sessions = state.find::<DynSessionStore>().cloned();
        let scorer = state.find::<DynSessionBehaviourScorer>().cloned();
        let analytics_repo = Arc::clone(&app_context.analytics_repositories().event_log);
        let route_classifier = Arc::clone(app_context.route_classifier());

        Self {
            sessions,
            scorer,
            analytics_repo,
            route_classifier,
            background: app_context.background_tasks().clone(),
        }
    }

    pub async fn track_request(
        &self,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let method = request.method().clone();
        let uri = request.uri().clone();

        let Some(req_ctx) = request.extensions().get::<RequestContext>().cloned() else {
            return Ok(next.run(request).await);
        };

        if !req_ctx.request.is_tracked {
            return Ok(next.run(request).await);
        }

        let user_agent = request
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let referer = request
            .headers()
            .get("referer")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let start_time = std::time::Instant::now();
        let response = next.run(request).await;
        let response_time_ms = start_time.elapsed().as_millis() as u64;
        let status_code = response.status();
        let html_response = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.trim_start().starts_with("text/html"));

        let should_track = self
            .route_classifier
            .should_track_analytics(uri.path(), method.as_str());

        let is_scanner =
            ScannerDetector::is_scanner(Some(uri.path()), user_agent.as_deref(), None, None);

        if should_track {
            self.spawn_tracking_tasks(TrackingParams {
                req_ctx: &req_ctx,
                uri: &uri,
                method: &method,
                status_code: status_code.as_u16(),
                response_time_ms,
                user_agent,
                referer,
                is_scanner,
                html_response,
            });
        }

        Ok(response)
    }

    fn spawn_tracking_tasks(&self, params: TrackingParams<'_>) {
        let req_ctx = params.req_ctx;
        let uri = params.uri;
        let method = params.method;
        let status_code = params.status_code;
        let response_time_ms = params.response_time_ms;
        let user_agent = params.user_agent;
        let referer = params.referer;
        let is_scanner = params.is_scanner;
        let html_response = params.html_response;
        let endpoint = format!("{} {}", method, uri.path());
        let path = uri.path().to_owned();

        if let Some(sessions) = &self.sessions {
            if is_scanner {
                spawn_mark_scanner_task(
                    &self.background,
                    Arc::clone(sessions),
                    req_ctx.request.session_id.clone(),
                );
            }
            spawn_velocity_scanner_check(
                &self.background,
                Arc::clone(sessions),
                req_ctx.request.session_id.clone(),
            );
            spawn_session_tracking_task(
                &self.background,
                Arc::clone(sessions),
                req_ctx.request.session_id.clone(),
            );
        }

        if let Some(scorer) = &self.scorer {
            spawn_behavioral_detection_task(
                &self.background,
                Arc::clone(scorer),
                DetectionSubject {
                    session_id: req_ctx.request.session_id.clone(),
                    fingerprint_hash: req_ctx.request.fingerprint_hash.clone(),
                    user_agent: user_agent.clone(),
                    request_count: 1,
                },
            );
        }

        events::spawn_analytics_event_task(
            &self.background,
            Arc::clone(&self.analytics_repo),
            Arc::clone(&self.route_classifier),
            AnalyticsEventParams {
                req_ctx: req_ctx.clone(),
                endpoint,
                path,
                method: method.to_string(),
                uri: uri.clone(),
                status_code,
                response_time_ms,
                user_agent,
                referer,
                html_response,
            },
        );
    }
}

fn spawn_session_tracking_task(
    background: &BackgroundTasks,
    sessions: DynSessionStore,
    session_id: SessionId,
) {
    // Why: one UPDATE per request — the increment also stamps
    // last_activity_at and duration_seconds.
    background.spawn("analytics_session_activity", async move {
        if let Err(e) = sessions.increment_request_count(&session_id).await {
            tracing::error!(error = %e, "Failed to record session request");
        }
    });
}

fn spawn_velocity_scanner_check(
    background: &BackgroundTasks,
    sessions: DynSessionStore,
    session_id: SessionId,
) {
    background.spawn("analytics_velocity_check", async move {
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

fn spawn_mark_scanner_task(
    background: &BackgroundTasks,
    sessions: DynSessionStore,
    session_id: SessionId,
) {
    background.spawn("analytics_mark_scanner", async move {
        if let Err(e) = sessions.mark_as_scanner(&session_id).await {
            tracing::warn!(error = %e, session_id = %session_id, "Failed to mark session as scanner");
        }
    });
}

fn spawn_behavioral_detection_task(
    background: &BackgroundTasks,
    scorer: DynSessionBehaviourScorer,
    subject: DetectionSubject,
) {
    background.spawn("analytics_behavioral_detection", async move {
        if let Err(e) = scorer.score(subject).await {
            tracing::error!(error = %e, "Failed to update behavioral detection");
        }
    });
}
