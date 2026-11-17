//! `OpenAI` streaming on the provider's wire (Chat Completions or Responses):
//! builds a canonical streaming request, posts it, and yields the shared
//! codec's canonical events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::StreamExt;
use systemprompt_wire::canonical::CanonicalTool;
use systemprompt_wire::{WireProtocol, openai_chat, openai_responses};

use crate::error::Result;
use crate::services::providers::canonical_bridge::{BridgeProvider, CanonicalBuild};
use crate::services::providers::{EventStream, GenerationParams};

use super::provider::OpenAiProvider;

impl OpenAiProvider {
    pub(super) async fn create_stream_request(
        &self,
        params: GenerationParams<'_>,
        tools: Option<Vec<CanonicalTool>>,
    ) -> Result<EventStream> {
        let canonical = CanonicalBuild::new(
            BridgeProvider::OpenAi,
            params.messages,
            params.model,
            params.max_output_tokens,
        )
        .with_sampling(params.sampling)
        .with_reasoning(params.reasoning)
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
        let stream = events.map(|event| event.map_err(crate::error::AiError::from));
        Ok(Box::pin(stream))
    }
}
