//! Gemini HTTP transport: client construction and the POST round trip.
//!
//! URL and auth come from the provider's upstream target: an API key rides
//! `x-goog-api-key` on the public endpoint, and a service account on Vertex AI
//! mints an OAuth bearer with `{project}` filled from its key. The path is the
//! shared dialect's (`?alt=sse` for the streaming method). Request-body
//! rendering and reply parsing live in the shared
//! `systemprompt_models::wire::gemini` codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::{Client, Response};
use serde_json::Value;
use systemprompt_models::services::WireProtocol;

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
    let call = provider.target.call().await?;
    let upstream = provider.upstream_model(model);
    let mut request = provider
        .client
        .post(call.url(WireProtocol::Gemini, upstream, stream));
    for (name, value) in call.headers(WireProtocol::Gemini) {
        request = request.header(name, value);
    }
    let response = request.json(body).send().await?;
    if !response.status().is_success() {
        return Err(crate::error::AiError::from_error_response("gemini", response).await);
    }
    Ok(response)
}
