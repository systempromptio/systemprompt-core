//! `OpenAI`-compatible text embeddings over `POST {endpoint}/embeddings`.
//!
//! The body and reply are rendered and read here rather than through the
//! canonical bridge because embeddings carry no messages; any OpenAI-shaped
//! base URL (Azure, `OpenRouter`, a local server) is reached the same way the
//! chat completions are, through the provider's upstream target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::ProviderId;
use systemprompt_wire::WireProtocol;

use crate::error::{AiError, ProviderError, Result};
use crate::models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

use super::provider::OpenAiProvider;

pub const EMBEDDINGS_PATH: &str = "/embeddings";

// Why: OpenAI Embeddings API request body —
// https://platform.openai.com/docs/api-reference/embeddings/create. `input`
// is sent as an array so one call embeds every text, and `dimensions` is
// only accepted by the text-embedding-3 models, so it is omitted unless set.
#[derive(Debug, Serialize)]
pub struct EmbeddingsBody<'a> {
    pub model: &'a str,
    pub input: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    pub encoding_format: &'static str,
}

// JSON: OpenAI `/embeddings` request body — upstream wire format.
pub fn build_request_body(request: &EmbeddingRequest, upstream_model: &str) -> Result<Value> {
    Ok(serde_json::to_value(EmbeddingsBody {
        model: upstream_model,
        input: &request.inputs,
        dimensions: request.dimensions,
        encoding_format: "float",
    })?)
}

// Why: OpenAI Embeddings API response object —
// https://platform.openai.com/docs/api-reference/embeddings/object. Vectors
// arrive in `data[]` with an explicit `index`, and the array is not
// guaranteed to be ordered, so the index is honoured when placing them.
#[derive(Debug, Deserialize)]
struct EmbeddingsReply {
    #[serde(default)]
    model: Option<String>,
    data: Vec<EmbeddingDatum>,
    #[serde(default)]
    usage: Option<ReplyUsage>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingDatum {
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct ReplyUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

// JSON: OpenAI `/embeddings` response body — upstream wire format.
pub fn parse_response(value: Value, request: &EmbeddingRequest) -> Result<EmbeddingResponse> {
    let reply: EmbeddingsReply = serde_json::from_value(value)?;
    let requested = request.inputs.len();
    if reply.data.len() != requested {
        return Err(AiError::Provider(ProviderError::EmbeddingCountMismatch {
            provider: ProviderId::new("openai"),
            requested,
            returned: reply.data.len(),
        }));
    }
    let mut vectors: Vec<Vec<f32>> = vec![Vec::new(); requested];
    for datum in reply.data {
        let Some(slot) = vectors.get_mut(datum.index) else {
            return Err(AiError::Provider(ProviderError::EmbeddingCountMismatch {
                provider: ProviderId::new("openai"),
                requested,
                returned: datum.index + 1,
            }));
        };
        *slot = datum.embedding;
    }
    let usage = reply
        .usage
        .map_or_else(EmbeddingUsage::default, |u| EmbeddingUsage {
            input_tokens: u.prompt_tokens,
            total_tokens: u.total_tokens,
        });
    Ok(EmbeddingResponse {
        model: reply.model.unwrap_or_else(|| request.model.clone()),
        vectors,
        usage,
    })
}

pub(super) async fn embed(
    provider: &OpenAiProvider,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse> {
    let upstream = provider.upstream_model(&request.model);
    let body = build_request_body(request, upstream)?;
    let call = provider.target.call().await?;
    let url = format!("{}{EMBEDDINGS_PATH}", call.endpoint().trim_end_matches('/'));
    let mut http = provider.client.post(url);
    for (name, value) in call.headers(WireProtocol::OpenAiChat) {
        http = http.header(name, value);
    }
    let response = http.json(&body).send().await?;
    if !response.status().is_success() {
        return Err(AiError::from_error_response("openai", response).await);
    }
    let value: Value = response.json().await?;
    let parsed = parse_response(value, request)?;
    tracing::debug!(
        model = %request.model,
        inputs = request.inputs.len(),
        input_tokens = parsed.usage.input_tokens,
        "openai embeddings returned"
    );
    Ok(parsed)
}
