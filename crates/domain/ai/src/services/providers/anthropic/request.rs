//! Shared request plumbing for the Anthropic Messages API — sampling
//! extraction and the POST/`status`-check round trip used by every driver
//! entry point.

use reqwest::Response;
use serde::Serialize;

use crate::error::Result;
use crate::models::ai::SamplingParams;

use super::provider::AnthropicProvider;

pub(super) type SamplingTuple = (Option<f32>, Option<f32>, Option<i32>, Option<Vec<String>>);

pub(super) fn sampling_tuple(sampling: Option<&SamplingParams>) -> SamplingTuple {
    sampling.map_or((None, None, None, None), |s| {
        (s.temperature, s.top_p, s.top_k, s.stop_sequences.clone())
    })
}

pub(super) async fn post_messages<T: Serialize + Sync>(
    provider: &AnthropicProvider,
    request: &T,
) -> Result<Response> {
    let response = provider
        .client
        .post(format!("{}/messages", provider.endpoint))
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(request)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(crate::error::AiError::from_error_response("anthropic", response).await);
    }

    Ok(response)
}
