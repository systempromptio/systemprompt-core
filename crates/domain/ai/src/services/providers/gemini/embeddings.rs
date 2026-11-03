//! Google text embeddings on the Gemini wire, for both of its hostings.
//!
//! The same model id (`text-embedding-005`, `gemini-embedding-001`) is served
//! by two different endpoints: Vertex AI exposes it as a publisher model on
//! `:predict`, the public Gemini API as `:batchEmbedContents`. The provider's
//! upstream target already knows which host it points at, so the hosting
//! picks the path, the body and the reply shape here and nothing else in the
//! provider changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::{Value, json};
use systemprompt_identifiers::ProviderId;
use systemprompt_wire::{Hosting, WireProtocol};

use crate::error::{AiError, ProviderError, Result};
use crate::models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

use super::provider::GeminiProvider;

#[must_use]
pub fn embeddings_path(hosting: Hosting, upstream_model: &str) -> String {
    match hosting {
        Hosting::Vertex => format!("/models/{upstream_model}:predict"),
        Hosting::FirstParty => format!("/models/{upstream_model}:batchEmbedContents"),
    }
}

// Why: Vertex AI text-embeddings request — one `instances[]` entry per text
// with `content`, and `parameters.outputDimensionality` to truncate the
// vector; https://cloud.google.com/vertex-ai/generative-ai/docs/model-reference/text-embeddings-api
// JSON: Vertex AI `:predict` request body — upstream wire format.
#[must_use]
pub fn build_vertex_body(request: &EmbeddingRequest) -> Value {
    let instances: Vec<Value> = request
        .inputs
        .iter()
        .map(|text| json!({ "content": text }))
        .collect();
    let mut body = json!({ "instances": instances });
    if let Some(dimensions) = request.dimensions {
        body["parameters"] = json!({ "outputDimensionality": dimensions });
    }
    body
}

// Why: Gemini API batchEmbedContents request — each entry names the model
// again as `models/<id>` and carries its own `outputDimensionality`;
// https://ai.google.dev/api/embeddings#method:-models.batchembedcontents
// JSON: Gemini `:batchEmbedContents` request body — upstream wire format.
#[must_use]
pub fn build_gemini_body(request: &EmbeddingRequest, upstream_model: &str) -> Value {
    let requests: Vec<Value> = request
        .inputs
        .iter()
        .map(|text| {
            let mut entry = json!({
                "model": format!("models/{upstream_model}"),
                "content": { "parts": [{ "text": text }] }
            });
            if let Some(dimensions) = request.dimensions {
                entry["outputDimensionality"] = json!(dimensions);
            }
            entry
        })
        .collect();
    json!({ "requests": requests })
}

// JSON: upstream request body for whichever hosting the target points at.
#[must_use]
pub fn build_request_body(hosting: Hosting, request: &EmbeddingRequest, upstream: &str) -> Value {
    match hosting {
        Hosting::Vertex => build_vertex_body(request),
        Hosting::FirstParty => build_gemini_body(request, upstream),
    }
}

// Why: Vertex AI text-embeddings response — `predictions[].embeddings.values`
// with `statistics.token_count` per instance;
// https://cloud.google.com/vertex-ai/generative-ai/docs/model-reference/text-embeddings-api#response_body
#[derive(Debug, Deserialize)]
struct VertexReply {
    predictions: Vec<VertexPrediction>,
}

#[derive(Debug, Deserialize)]
struct VertexPrediction {
    embeddings: VertexEmbedding,
}

#[derive(Debug, Deserialize)]
struct VertexEmbedding {
    values: Vec<f32>,
    #[serde(default)]
    statistics: Option<VertexStatistics>,
}

#[derive(Debug, Deserialize)]
struct VertexStatistics {
    #[serde(default)]
    token_count: u32,
}

// Why: Gemini API batchEmbedContents response — `embeddings[].values` in
// request order and no usage block;
// https://ai.google.dev/api/embeddings#response-body_1
#[derive(Debug, Deserialize)]
struct GeminiReply {
    embeddings: Vec<GeminiEmbedding>,
}

#[derive(Debug, Deserialize)]
struct GeminiEmbedding {
    values: Vec<f32>,
}

// JSON: Vertex `:predict` or Gemini `:batchEmbedContents` response body —
// upstream wire format per hosting.
pub fn parse_response(
    hosting: Hosting,
    value: Value,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse> {
    let (vectors, input_tokens) = match hosting {
        Hosting::Vertex => {
            let reply: VertexReply = serde_json::from_value(value)?;
            let tokens = reply
                .predictions
                .iter()
                .filter_map(|p| p.embeddings.statistics.as_ref())
                .map(|s| s.token_count)
                .sum::<u32>();
            let vectors = reply
                .predictions
                .into_iter()
                .map(|p| p.embeddings.values)
                .collect::<Vec<_>>();
            (vectors, tokens)
        },
        Hosting::FirstParty => {
            let reply: GeminiReply = serde_json::from_value(value)?;
            let vectors = reply
                .embeddings
                .into_iter()
                .map(|e| e.values)
                .collect::<Vec<_>>();
            (vectors, 0)
        },
    };
    if vectors.len() != request.inputs.len() {
        return Err(AiError::Provider(ProviderError::EmbeddingCountMismatch {
            provider: ProviderId::new("gemini"),
            requested: request.inputs.len(),
            returned: vectors.len(),
        }));
    }
    Ok(EmbeddingResponse {
        model: request.model.clone(),
        vectors,
        usage: EmbeddingUsage {
            input_tokens,
            total_tokens: input_tokens,
        },
    })
}

pub(super) async fn embed(
    provider: &GeminiProvider,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse> {
    let call = provider.target.call().await?;
    let hosting = call.hosting();
    let upstream = provider.upstream_model(&request.model);
    let body = build_request_body(hosting, request, upstream);
    let url = format!(
        "{}{}",
        call.endpoint().trim_end_matches('/'),
        embeddings_path(hosting, upstream)
    );
    let mut http = provider.client.post(url);
    for (name, value) in call.headers(WireProtocol::Gemini) {
        http = http.header(name, value);
    }
    let response = http.json(&body).send().await?;
    if !response.status().is_success() {
        return Err(AiError::from_error_response("gemini", response).await);
    }
    let value: Value = response.json().await?;
    let parsed = parse_response(hosting, value, request)?;
    tracing::debug!(
        model = %request.model,
        hosting = %hosting,
        inputs = request.inputs.len(),
        input_tokens = parsed.usage.input_tokens,
        "gemini embeddings returned"
    );
    Ok(parsed)
}
