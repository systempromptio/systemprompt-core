//! `CloudApiClient` constructor + accessors. Lower-level HTTP verbs
//! live in `methods.rs`; high-level endpoints in `endpoints.rs`.

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};

use super::types::ApiError;
use crate::error::{CloudError, CloudResult};

/// Bearer-token authenticated client for the systemprompt.io Cloud
/// API.
#[derive(Debug)]
pub struct CloudApiClient {
    pub(super) client: Client,
    pub(super) api_url: String,
    pub(super) token: String,
}

impl CloudApiClient {
    /// Build a new client pointing at `api_url` with the given bearer
    /// `token`.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`reqwest::Error`] if the HTTP client
    /// cannot be constructed (e.g. invalid TLS configuration).
    pub fn new(api_url: &str, token: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_DEFAULT_TIMEOUT)
                .build()?,
            api_url: api_url.to_string(),
            token: token.to_string(),
        })
    }

    /// Borrow the API base URL.
    #[must_use]
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Borrow the configured bearer token.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    pub(super) async fn handle_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> CloudResult<T> {
        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthorized);
        }

        if !status.is_success() {
            return Err(parse_error_response(status, response).await);
        }

        response.json().await.map_err(CloudError::from)
    }

    pub(super) async fn handle_no_content_response(
        &self,
        response: reqwest::Response,
    ) -> CloudResult<()> {
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(CloudError::Unauthorized);
        }
        if status == StatusCode::NO_CONTENT || status.is_success() {
            return Ok(());
        }
        Err(parse_error_response(status, response).await)
    }
}

pub(super) async fn parse_error_response(
    status: StatusCode,
    response: reqwest::Response,
) -> CloudError {
    let error_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to read error response body");
            String::from("<failed to read response body>")
        },
    };

    serde_json::from_str::<ApiError>(&error_text).map_or_else(
        |_| CloudError::HttpStatus {
            status: status.as_u16(),
            body: error_text.chars().take(500).collect(),
        },
        |parsed| CloudError::ApiError {
            message: format!("{}: {}", parsed.error.code, parsed.error.message),
        },
    )
}
