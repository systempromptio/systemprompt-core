//! Anthropic streaming: builds a canonical streaming request, posts it, and
//! yields the shared codec's canonical events. SSE framing is the shared
//! [`anthropic::sse_to_canonical_events`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::StreamExt;
use systemprompt_wire::anthropic;
use systemprompt_wire::canonical::CanonicalTool;

use crate::error::{AiError, Result};
use crate::services::providers::canonical_bridge::{BridgeProvider, CanonicalBuild};
use crate::services::providers::{EventStream, GenerationParams};

use super::provider::AnthropicProvider;
use super::request::post_body;

impl AnthropicProvider {
    pub(super) async fn create_stream_request(
        &self,
        params: GenerationParams<'_>,
        tools: Option<Vec<CanonicalTool>>,
    ) -> Result<EventStream> {
        let canonical = CanonicalBuild::new(
            BridgeProvider::Anthropic,
            params.messages,
            params.model,
            params.max_output_tokens,
        )
        .with_sampling(params.sampling)
        .with_reasoning(params.reasoning)
        .with_tools(tools.unwrap_or_default())
        .with_stream(true)
        .into_request();

        let upstream = self.upstream_model(params.model);
        let body = anthropic::build_request_body(&canonical, upstream, None);
        let response = post_body(self, body, upstream, true).await?;

        let stream = anthropic::sse_to_canonical_events(response.bytes_stream())
            .map(|event| event.map_err(AiError::from));

        Ok(Box::pin(stream))
    }
}
