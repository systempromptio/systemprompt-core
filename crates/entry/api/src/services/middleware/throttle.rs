//! Throttle middleware for dynamic rate limit enforcement based on behavioral
//! bot detection.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use systemprompt_core_analytics::{SessionRepository, ThrottleLevel};
use systemprompt_core_database::DbPool;
use systemprompt_models::RequestContext;

/// Middleware for checking and enforcing throttle levels on incoming requests.
///
/// This middleware integrates with the behavioral bot detection system to
/// enforce rate limiting escalation. Sessions flagged as behavioral bots or
/// exhibiting suspicious patterns will have their throttle level increased,
/// resulting in reduced request rates or complete blocking.
#[derive(Debug, Clone)]
pub struct ThrottleMiddleware {
    session_repo: Arc<SessionRepository>,
}

impl ThrottleMiddleware {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            session_repo: Arc::new(SessionRepository::new(db_pool)),
        }
    }

    /// Check the throttle level for the current request and enforce rate
    /// limits.
    ///
    /// Returns:
    /// - 200 OK: Request proceeds normally
    /// - 429 Too Many Requests: Session is blocked (throttle_level = 3)
    ///
    /// For throttled sessions (level 1-2), requests proceed but with a header
    /// indicating the throttle level for client awareness.
    pub async fn check_throttle(
        &self,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let Some(req_ctx) = request.extensions().get::<RequestContext>().cloned() else {
            return Ok(next.run(request).await);
        };

        if !req_ctx.request.is_tracked {
            return Ok(next.run(request).await);
        }

        let throttle_level = self
            .session_repo
            .get_throttle_level(&req_ctx.request.session_id)
            .await
            .unwrap_or(0);

        let level = ThrottleLevel::from(throttle_level);

        if !level.allows_requests() {
            return Ok((
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("Retry-After", "3600"),
                    ("X-Throttle-Level", "blocked"),
                    ("X-Throttle-Reason", "behavioral_bot_detection"),
                ],
                "Request blocked due to suspicious activity",
            )
                .into_response());
        }

        let mut response = next.run(request).await;

        if throttle_level > 0 {
            let level_str = match throttle_level {
                1 => "warning",
                2 => "severe",
                _ => "unknown",
            };

            if let Ok(header_value) = level_str.parse() {
                response
                    .headers_mut()
                    .insert("X-Throttle-Level", header_value);
            }

            let multiplier = level.rate_multiplier();
            if let Ok(header_value) = format!("{multiplier}").parse() {
                response
                    .headers_mut()
                    .insert("X-Rate-Multiplier", header_value);
            }
        }

        Ok(response)
    }
}

/// Standalone function for use with axum middleware layer.
///
/// Use this when you need to add throttle checking as a layer:
/// ```ignore
/// Router::new()
///     .layer(axum::middleware::from_fn_with_state(
///         throttle_middleware.clone(),
///         check_throttle_level
///     ))
/// ```
pub async fn check_throttle_level(
    middleware: axum::extract::State<ThrottleMiddleware>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    middleware.check_throttle(request, next).await
}
