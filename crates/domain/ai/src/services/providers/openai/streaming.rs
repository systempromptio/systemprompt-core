//! `OpenAI` Chat Completions streaming: builds a canonical streaming request,
//! posts it, and maps the shared codec's canonical events into agent
//! [`StreamChunk`]s.

use std::pin::Pin;

use futures::{Stream, StreamExt};
use systemprompt_models::wire::canonical::CanonicalTool;
use systemprompt_models::wire::openai_chat;

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

        let body = openai_chat::build_request_body(&canonical, params.model, None);
        let response = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(crate::error::AiError::from_error_response("openai", response).await);
        }

        let events =
            openai_chat::sse_to_canonical_events(response.bytes_stream(), params.model.to_owned());
        let stream = events.filter_map(|result| async move {
            match result {
                Ok(event) => canonical_bridge::event_to_chunk(event).map(Ok),
                Err(e) => Some(Err(crate::error::AiError::Internal(e))),
            }
        });
        Ok(Box::pin(stream))
    }
}
