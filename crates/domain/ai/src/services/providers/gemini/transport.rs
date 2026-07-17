//! Gemini HTTP transport: client construction and the POST round trip.
//!
//! Auth uses the official `x-goog-api-key` header and the path is built by the
//! shared codec's [`gemini::upstream_path`] (which appends `?alt=sse` for the
//! streaming method). Request-body rendering and reply parsing live in the
//! shared `systemprompt_models::wire::gemini` codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::{Client, Response};
use serde_json::Value;
use systemprompt_models::wire::gemini;

use super::constants::timeout;
use super::provider::GeminiProvider;
use crate::error::Result;

pub(super) fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(systemprompt_models::net::AI_PROVIDER_REQUEST_TIMEOUT)
        .connect_timeout(timeout::CONNECT_TIMEOUT)
        .build()
        .map_err(|e| crate::error::AiError::Internal(format!("Failed to create HTTP client: {e}")))
}

pub(super) async fn post(
    provider: &GeminiProvider,
    body: &Value,
    model: &str,
    stream: bool,
) -> Result<Response> {
    let url = format!(
        "{}{}",
        provider.endpoint,
        gemini::upstream_path(model, stream)
    );
    let response = provider
        .client
        .post(&url)
        .header(gemini::API_KEY_HEADER, &provider.api_key)
        .json(body)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(crate::error::AiError::from_error_response("gemini", response).await);
    }
    Ok(response)
}
