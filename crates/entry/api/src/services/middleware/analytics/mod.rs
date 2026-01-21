mod detection;
mod events;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

use systemprompt_analytics::SessionRepository;
use systemprompt_identifiers::SessionId;
use systemprompt_logging::AnalyticsRepository;
use systemprompt_models::{RequestContext, RouteClassifier};
use systemprompt_runtime::AppContext;
use systemprompt_security::ScannerDetector;

pub use events::AnalyticsEventParams;

#[derive(Debug, Clone)]
pub struct AnalyticsMiddleware {
    session_repo: Arc<SessionRepository>,
    analytics_repo: Arc<AnalyticsRepository>,
    route_classifier: Arc<RouteClassifier>,
}

impl AnalyticsMiddleware {
    pub fn new(app_context: &AppContext) -> Self {
        let db_pool = app_context.db_pool().clone();
        let session_repo = Arc::new(SessionRepository::new(db_pool.clone()));
        let analytics_repo = Arc::new(AnalyticsRepository::new(db_pool.clone()));
        let route_classifier = app_context.route_classifier().clone();

        Self {
            session_repo,
            analytics_repo,
            route_classifier,
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
            .map(ToString::to_string);

        let referer = request
            .headers()
            .get("referer")
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);

        let start_time = std::time::Instant::now();
        let response = next.run(request).await;
        let response_time_ms = start_time.elapsed().as_millis() as u64;
        let status_code = response.status();

        let should_track = self
            .route_classifier
            .should_track_analytics(uri.path(), method.as_str());

        let is_scanner =
            ScannerDetector::is_scanner(Some(uri.path()), user_agent.as_deref(), None, None);

        if should_track {
            self.spawn_tracking_tasks(
                &req_ctx,
                &uri,
                &method,
                status_code.as_u16(),
                response_time_ms,
                user_agent,
                referer,
                is_scanner,
            );
        }

        Ok(response)
    }

    fn spawn_tracking_tasks(
        &self,
        req_ctx: &RequestContext,
        uri: &http::Uri,
        method: &http::Method,
        status_code: u16,
        response_time_ms: u64,
        user_agent: Option<String>,
        referer: Option<String>,
        is_scanner: bool,
    ) {
        let endpoint = format!("{} {}", method, uri.path());
        let path = uri.path().to_string();

        if is_scanner {
            self.spawn_mark_scanner_task(req_ctx.request.session_id.clone());
        }

        self.spawn_session_tracking_task(req_ctx.request.session_id.clone());

        detection::spawn_behavioral_detection_task(
            self.session_repo.clone(),
            req_ctx.request.session_id.clone(),
            None,
            user_agent.clone(),
            1,
        );

        events::spawn_analytics_event_task(
            self.analytics_repo.clone(),
            self.route_classifier.clone(),
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
            },
        );
    }

    fn spawn_session_tracking_task(&self, session_id: SessionId) {
        let session_repo = self.session_repo.clone();

        tokio::spawn(async move {
            if let Err(e) = session_repo.update_activity(&session_id).await {
                tracing::error!(error = %e, "Failed to update session activity");
            }

            if let Err(e) = session_repo.increment_request_count(&session_id).await {
                tracing::error!(error = %e, "Failed to increment request count");
            }
        });
    }

    fn spawn_mark_scanner_task(&self, session_id: SessionId) {
        let session_repo = self.session_repo.clone();

        tokio::spawn(async move {
            if let Err(e) = session_repo.mark_as_scanner(&session_id).await {
                tracing::warn!(error = %e, session_id = %session_id, "Failed to mark session as scanner");
            }
        });
    }
}
