//! Anthropic streaming: builds a canonical streaming request, posts it, and
//! maps the shared codec's canonical events into agent [`StreamChunk`]s. SSE
//! framing is the shared [`anthropic::sse_to_canonical_events`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use futures::{Stream, StreamExt};
use systemprompt_models::wire::anthropic;
use systemprompt_models::wire::canonical::CanonicalTool;

use crate::error::{AiError, Result};
use crate::models::ai::StreamChunk;
use crate::services::providers::GenerationParams;
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

use super::provider::AnthropicProvider;
use super::request::post_body;

impl AnthropicProvider {
    pub(super) async fn create_stream_request(
        &self,
        params: GenerationParams<'_>,
        tools: Option<Vec<CanonicalTool>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let canonical = CanonicalBuild::new(
            BridgeProvider::Anthropic,
            params.messages,
            params.model,
            params.max_output_tokens,
        )
        .with_sampling(params.sampling)
        .with_tools(tools.unwrap_or_default())
        .with_stream(true)
        .into_request();

        let upstream = self.upstream_model(params.model);
        let body = anthropic::build_request_body(&canonical, upstream, None);
        let response = post_body(self, body, upstream, true).await?;

        let stream =
            anthropic::sse_to_canonical_events(response.bytes_stream()).filter_map(|event| {
                futures::future::ready(match event {
                    Ok(event) => canonical_bridge::event_to_chunk(event).map(Ok),
                    Err(e) => Some(Err(AiError::Internal(format!("Stream error: {e}")))),
                })
            });

        Ok(Box::pin(stream))
    }
}
