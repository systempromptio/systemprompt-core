use axum::http::Method;
use systemprompt_models::Config;
use thiserror::Error;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Debug, Error)]
pub enum CorsError {
    #[error("Invalid origin '{origin}' in cors_allowed_origins: {reason}")]
    InvalidOrigin { origin: String, reason: String },
    #[error("cors_allowed_origins must contain at least one valid origin")]
    EmptyOrigins,
}

#[derive(Debug, Clone, Copy)]
pub struct CorsMiddleware;

impl CorsMiddleware {
    pub fn build_layer(config: &Config) -> Result<CorsLayer, CorsError> {
        let mut origins = Vec::new();
        for origin in &config.cors_allowed_origins {
            let trimmed = origin.trim();
            if trimmed.is_empty() {
                continue;
            }
            let header_value =
                trimmed
                    .parse::<http::HeaderValue>()
                    .map_err(|e| CorsError::InvalidOrigin {
                        origin: origin.clone(),
                        reason: e.to_string(),
                    })?;
            origins.push(header_value);
        }

        if origins.is_empty() {
            return Err(CorsError::EmptyOrigins);
        }

        Ok(CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_credentials(true)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                http::header::AUTHORIZATION,
                http::header::CONTENT_TYPE,
                http::header::ACCEPT,
                http::header::ORIGIN,
                http::header::ACCESS_CONTROL_REQUEST_METHOD,
                http::header::ACCESS_CONTROL_REQUEST_HEADERS,
                http::HeaderName::from_static("mcp-protocol-version"),
                http::HeaderName::from_static("x-context-id"),
                http::HeaderName::from_static("x-trace-id"),
                http::HeaderName::from_static("x-call-source"),
            ])
            .expose_headers([http::header::WWW_AUTHENTICATE]))
    }
}
