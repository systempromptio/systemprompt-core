use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use systemprompt_analytics::{SessionRepository, ThrottleLevel};
use systemprompt_database::DbPool;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_models::RequestContext;

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

    pub async fn check_throttle(&self, request: Request, next: Next) -> Result<Response, ApiError> {
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
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, session_id = %req_ctx.request.session_id, "Failed to get throttle level");
                0
            });

        let level = ThrottleLevel::from(throttle_level);

        if !level.allows_requests() {
            let api_error = ApiError::new(
                ErrorCode::RateLimited,
                "Request blocked due to suspicious activity",
            );
            let mut response = api_error.into_response();
            response
                .headers_mut()
                .insert("Retry-After", "3600".parse().expect("valid header value"));
            response.headers_mut().insert(
                "X-Throttle-Level",
                "blocked".parse().expect("valid header value"),
            );
            response.headers_mut().insert(
                "X-Throttle-Reason",
                "behavioral_bot_detection"
                    .parse()
                    .expect("valid header value"),
            );
            return Ok(response);
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

pub async fn check_throttle_level(
    middleware: axum::extract::State<ThrottleMiddleware>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    middleware.check_throttle(request, next).await
}
