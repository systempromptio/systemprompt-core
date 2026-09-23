//! HTTP plumbing for the Anthropic Messages API: the POST + status-check round
//! trip shared by every driver entry point. Where the request goes and how it
//! authenticates come from the provider's [`UpstreamTarget`], so the same
//! driver reaches `api.anthropic.com` and Claude on Vertex AI alike; the body
//! is rendered by the shared `systemprompt_models::wire` codec.
//!
//! [`UpstreamTarget`]: crate::services::upstream::UpstreamTarget
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Response;
use serde_json::Value;
use systemprompt_models::services::WireProtocol;

use super::provider::AnthropicProvider;
use crate::error::Result;

pub(super) async fn post_body(
    provider: &AnthropicProvider,
    mut body: Value,
    upstream_model: &str,
    stream: bool,
) -> Result<Response> {
    let call = provider.target.call().await?;
    call.finish_value(WireProtocol::Anthropic, &mut body);
    let mut request =
        provider
            .client
            .post(call.url(WireProtocol::Anthropic, upstream_model, stream));
    for (name, value) in call.headers(WireProtocol::Anthropic) {
        request = request.header(name, value);
    }
    let response = request.json(&body).send().await?;
    if !response.status().is_success() {
        return Err(crate::error::AiError::from_error_response("anthropic", response).await);
    }
    Ok(response)
}
