use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use reqwest::Method;
use std::str::FromStr;
use systemprompt_models::RequestContext;
use systemprompt_traits::InjectContextHeaders;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("Service '{service}' not found in inventory")]
    ServiceNotFound { service: String },

    #[error("Service '{service}' is not running (status: {status})")]
    ServiceNotRunning { service: String, status: String },

    #[error("Failed to connect to {service} at {url}: {source}")]
    ConnectionFailed {
        service: String,
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("Request to {service} timed out")]
    Timeout { service: String },

    #[error("Invalid response from {service}: {reason}")]
    InvalidResponse { service: String, reason: String },

    #[error("Failed to build URL for {service}: {reason}")]
    UrlConstructionFailed { service: String, reason: String },

    #[error("Failed to extract request body: {source}")]
    BodyExtractionFailed {
        #[source]
        source: axum::Error,
    },

    #[error("Invalid HTTP method: {reason}")]
    InvalidMethod { reason: String },

    #[error("Database error when looking up service '{service}': {source}")]
    DatabaseError {
        service: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Authentication required for service '{service}'")]
    AuthenticationRequired { service: String },

    #[error("OAuth challenge response")]
    AuthChallenge(Response<Body>),

    #[error("Access forbidden for service '{service}'")]
    Forbidden { service: String },

    #[error("Missing request context: {message}")]
    MissingContext { message: String },
}

impl ProxyError {
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            Self::ServiceNotFound { .. } => StatusCode::NOT_FOUND,
            Self::ServiceNotRunning { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::ConnectionFailed { .. } | Self::InvalidResponse { .. } => StatusCode::BAD_GATEWAY,
            Self::Timeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::UrlConstructionFailed { .. } | Self::DatabaseError { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            Self::BodyExtractionFailed { .. } | Self::InvalidMethod { .. } => {
                StatusCode::BAD_REQUEST
            },
            Self::AuthenticationRequired { .. } | Self::MissingContext { .. } => {
                StatusCode::UNAUTHORIZED
            },
            Self::AuthChallenge(response) => response.status(),
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
        }
    }
}

impl From<ProxyError> for StatusCode {
    fn from(error: ProxyError) -> Self {
        error.to_status_code()
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        match self {
            Self::AuthChallenge(response) => response.into_response(),
            other => {
                let status = other.to_status_code();
                let message = other.to_string();
                (status, message).into_response()
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeaderInjector;

impl HeaderInjector {
    pub fn inject_context(headers: &mut HeaderMap, req_ctx: &RequestContext) {
        req_ctx.inject_headers(headers);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UrlResolver;

impl UrlResolver {
    pub fn build_backend_url(protocol: &str, host: &str, port: i32, path: &str) -> String {
        let clean_path = path.trim_start_matches('/');

        match protocol {
            "mcp" => {
                if clean_path.is_empty() || clean_path == "mcp" {
                    format!("http://{host}:{port}/mcp")
                } else {
                    format!("http://{host}:{port}/{clean_path}")
                }
            },
            _ => {
                if clean_path.is_empty() {
                    format!("http://{host}:{port}/")
                } else {
                    format!("http://{host}:{port}/{clean_path}")
                }
            },
        }
    }

    pub fn append_query_params(url: String, query: Option<&str>) -> String {
        match query {
            Some(q) if !q.is_empty() => format!("{url}?{q}"),
            _ => url,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RequestBuilder;

impl RequestBuilder {
    pub async fn extract_body(body: Body) -> Result<Vec<u8>, axum::Error> {
        const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

        to_bytes(body, MAX_BODY_SIZE)
            .await
            .map(|bytes| bytes.to_vec())
    }

    pub fn parse_method(method_str: &str) -> Result<Method, String> {
        Method::from_str(method_str)
            .map_err(|e| format!("Invalid HTTP method '{}': {}", method_str, e))
    }

    pub fn build_request(
        client: &reqwest::Client,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Vec<u8>,
    ) -> Result<reqwest::RequestBuilder, StatusCode> {
        let mut req_builder = client.request(method, url);
        req_builder = Self::add_headers(req_builder, headers);

        if !body.is_empty() {
            req_builder = req_builder.body(body);
        }

        Ok(req_builder)
    }

    fn add_headers(
        mut req_builder: reqwest::RequestBuilder,
        headers: &HeaderMap,
    ) -> reqwest::RequestBuilder {
        for (key, value) in headers {
            if let Ok(value_str) = value.to_str() {
                let key_str = key.as_str();

                if Self::should_skip_header(key_str) {
                    continue;
                }

                if key_str.eq_ignore_ascii_case("authorization") {
                    if Self::is_valid_auth_header(value_str) {
                        req_builder = req_builder.header(key_str, value_str);
                    }
                } else {
                    req_builder = req_builder.header(key_str, value_str);
                }
            }
        }
        req_builder
    }

    fn should_skip_header(header_name: &str) -> bool {
        let lower_name = header_name.to_lowercase();
        matches!(lower_name.as_str(), "host" | "x-mcp-proxy-auth")
    }

    fn is_valid_auth_header(value: &str) -> bool {
        value != "Bearer" && !value.trim().eq_ignore_ascii_case("bearer")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResponseHandler;

impl ResponseHandler {
    pub fn build_response(response: reqwest::Response) -> Result<Response<Body>, String> {
        let status_code = response.status().as_u16();
        let axum_status = StatusCode::from_u16(status_code)
            .map_err(|e| format!("Invalid status code {}: {}", status_code, e))?;

        let response_headers = response.headers().clone();

        let stream = response.bytes_stream().map_err(std::io::Error::other);
        let body = Body::from_stream(stream);

        let mut axum_response = Response::builder().status(axum_status);

        for (key, value) in &response_headers {
            let key_str = key.as_str();
            if let Ok(value_str) = value.to_str() {
                if Self::should_preserve_header(key_str) {
                    axum_response = axum_response.header(key_str, value_str);
                }
            }
        }

        axum_response = axum_response
            .header("connection", "keep-alive")
            .header("cache-control", "no-cache");

        axum_response
            .body(body)
            .map_err(|e| format!("Failed to build response body: {}", e))
    }

    fn should_preserve_header(key: &str) -> bool {
        match key.to_lowercase().as_str() {
            "host" | "authorization" | "proxy-authorization" | "upgrade" | "te" => false,
            header if header.starts_with("x-mcp-") => true,
            _ => true,
        }
    }
}
