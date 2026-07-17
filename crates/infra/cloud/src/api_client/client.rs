//! `CloudApiClient` constructor + accessors. Lower-level HTTP verbs
//! live in `methods.rs`; high-level endpoints in `endpoints.rs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use systemprompt_models::net::{HTTP_CONNECT_TIMEOUT, HTTP_DEFAULT_TIMEOUT};
use tokio::sync::Mutex;

use super::types::ApiError;
use crate::error::{CloudError, CloudResult};

pub(super) type TenantTokenCache = Arc<Mutex<Option<(String, Instant)>>>;

#[derive(Debug)]
pub struct CloudApiClient {
    pub(super) client: Client,
    pub(super) api_url: String,
    pub(super) token: String,
    pub(super) tenant_token_cache: TenantTokenCache,
}

impl CloudApiClient {
    pub fn new(api_url: &str, token: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .timeout(HTTP_DEFAULT_TIMEOUT)
                .build()?,
            api_url: api_url.to_owned(),
            token: token.to_owned(),
            tenant_token_cache: Arc::new(Mutex::new(None)),
        })
    }

    #[must_use]
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

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
