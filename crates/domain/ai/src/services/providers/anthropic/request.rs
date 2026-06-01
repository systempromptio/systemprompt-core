//! HTTP plumbing for the Anthropic Messages API: the POST + status-check round
//! trip shared by every driver entry point. Auth headers and request-body
//! rendering come from the shared `systemprompt_models::wire` codec.

use reqwest::Response;
use serde_json::Value;
use systemprompt_models::wire::anthropic;

use super::provider::AnthropicProvider;
use crate::error::Result;

pub(super) async fn post_body(provider: &AnthropicProvider, body: &Value) -> Result<Response> {
    let mut request = provider
        .client
        .post(format!("{}/messages", provider.endpoint));
    for (name, value) in anthropic::auth_headers(&provider.api_key) {
        request = request.header(name, value);
    }
    let response = request.json(body).send().await?;
    if !response.status().is_success() {
        return Err(crate::error::AiError::from_error_response("anthropic", response).await);
    }
    Ok(response)
}
