//! Gemini streaming: builds a canonical streaming request, posts it to
//! `streamGenerateContent?alt=sse`, and maps the shared codec's canonical
//! events into agent [`StreamChunk`]s.

use std::pin::Pin;

use futures::stream::{Stream, StreamExt};
use systemprompt_models::wire::gemini;

use crate::error::Result;
use crate::models::ai::StreamChunk;
use crate::services::providers::GenerationParams;
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

use super::provider::GeminiProvider;
use super::transport;

pub(super) async fn generate_stream(
    provider: &GeminiProvider,
    params: GenerationParams<'_>,
) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
    let canonical = CanonicalBuild::new(
        BridgeProvider::Gemini,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .with_stream(true)
    .into_request();

    let body = gemini::build_request_body(&canonical);
    let response = transport::post(provider, &body, params.model, true).await?;

    let events = gemini::sse_to_canonical_events(response.bytes_stream(), params.model.to_owned());
    let stream = events.filter_map(|result| async move {
        match result {
            Ok(event) => canonical_bridge::event_to_chunk(event).map(Ok),
            Err(e) => Some(Err(crate::error::AiError::Internal(e))),
        }
    });
    Ok(Box::pin(stream))
}
