//! What every embedding driver shares.
//!
//! The per-call input cap each upstream enforces, splitting a request into
//! calls under it and joining the replies back in input order, and the token
//! estimate used when an upstream reply carries no usage block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use systemprompt_manifest::services::estimate_text_tokens;

use crate::error::Result;
use crate::models::ai::{EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};

// Why: OpenAI Embeddings API — `input` as an array "must not exceed 2048
// elements"; https://platform.openai.com/docs/api-reference/embeddings/create
pub const OPENAI_MAX_INPUTS: usize = 2048;

// Why: Vertex AI text-embeddings `:predict` accepts at most 250 `instances`
// per request;
// https://cloud.google.com/vertex-ai/generative-ai/docs/model-reference/text-embeddings-api#request_body
pub const VERTEX_MAX_INSTANCES: usize = 250;

// Why: Gemini API `batchEmbedContents` accepts at most 100 `requests` per
// call; https://ai.google.dev/api/embeddings#method:-models.batchembedcontents
pub const GEMINI_MAX_BATCH: usize = 100;

#[must_use]
pub fn estimated_usage(inputs: &[String]) -> EmbeddingUsage {
    EmbeddingUsage::from_estimate(estimate_text_tokens(inputs.iter().map(String::as_str)))
}

pub async fn embed_in_batches<F, Fut>(
    request: &EmbeddingRequest,
    max_inputs: usize,
    call: F,
) -> Result<EmbeddingResponse>
where
    F: Fn(EmbeddingRequest) -> Fut,
    Fut: Future<Output = Result<EmbeddingResponse>>,
{
    if request.inputs.is_empty() {
        return Ok(EmbeddingResponse::empty(request.model.clone()));
    }
    let mut joined: Option<EmbeddingResponse> = None;
    for chunk in request.inputs.chunks(max_inputs.max(1)) {
        let reply = call(request.with_inputs(chunk.to_vec())).await?;
        joined = Some(match joined {
            None => reply,
            Some(mut acc) => {
                acc.vectors.extend(reply.vectors);
                acc.usage = acc.usage.summed_with(reply.usage);
                acc
            },
        });
    }
    Ok(joined.unwrap_or_else(|| EmbeddingResponse::empty(request.model.clone())))
}
