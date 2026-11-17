//! Gemini streaming: builds a canonical streaming request, posts it to
//! `streamGenerateContent?alt=sse`, and yields the shared codec's canonical
//! events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::stream::StreamExt;
use systemprompt_wire::gemini;

use crate::error::Result;
use crate::services::providers::canonical_bridge::{BridgeProvider, CanonicalBuild};
use crate::services::providers::{EventStream, GenerationParams};

use super::provider::GeminiProvider;
use super::transport;

pub(super) async fn generate_stream(
    provider: &GeminiProvider,
    params: GenerationParams<'_>,
) -> Result<EventStream> {
    let canonical = CanonicalBuild::new(
        BridgeProvider::Gemini,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .with_reasoning(params.reasoning)
    .with_stream(true)
    .into_request();

    let body = gemini::build_request_body(&canonical, None);
    let response = transport::post(provider, &body, params.model, true).await?;

    let events = gemini::sse_to_canonical_events(response.bytes_stream(), params.model.to_owned());
    let stream = events.map(|event| event.map_err(crate::error::AiError::from));
    Ok(Box::pin(stream))
}
