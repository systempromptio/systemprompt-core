//! `OpenAI` streaming on the provider's wire (Chat Completions or Responses):
//! builds a canonical streaming request, posts it, and maps the shared codec's
//! canonical events into agent [`StreamChunk`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use futures::{Stream, StreamExt};
use systemprompt_models::services::WireProtocol;
use systemprompt_models::wire::canonical::CanonicalTool;
use systemprompt_models::wire::{openai_chat, openai_responses};

use crate::error::Result;
use crate::models::ai::StreamChunk;
use crate::services::providers::GenerationParams;
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

use super::provider::OpenAiProvider;

impl OpenAiProvider {
    pub(super) async fn create_stream_request(
        &self,
        params: GenerationParams<'_>,
        tools: Option<Vec<CanonicalTool>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let canonical = CanonicalBuild::new(
            BridgeProvider::OpenAi,
            params.messages,
            params.model,
            params.max_output_tokens,
        )
        .with_sampling(params.sampling)
        .with_tools(tools.unwrap_or_default())
        .with_stream(true)
        .into_request();

        let wire = self.target.wire();
        let upstream = self.upstream_model(params.model);
        let body = self.render(&canonical, upstream);
        let response = self.post(wire, body, upstream, true).await?;

        let events = match wire {
            WireProtocol::OpenAiResponses => openai_responses::sse_to_canonical_events(
                response.bytes_stream(),
                params.model.to_owned(),
            ),
            _ => openai_chat::sse_to_canonical_events(
                response.bytes_stream(),
                params.model.to_owned(),
            ),
        };
        let stream = events.filter_map(|result| async move {
            match result {
                Ok(event) => canonical_bridge::event_to_chunk(event).map(Ok),
                Err(e) => Some(Err(crate::error::AiError::Internal(e))),
            }
        });
        Ok(Box::pin(stream))
    }
}
