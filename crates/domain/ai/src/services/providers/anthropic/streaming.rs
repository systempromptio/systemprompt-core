//! Anthropic streaming: builds a canonical streaming request, posts it, frames
//! the SSE byte stream, and maps each decoded frame through the shared codec's
//! [`anthropic::event_from_sse`] into agent [`StreamChunk`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use futures::{Stream, StreamExt};
use serde_json::Value;
use systemprompt_models::wire::anthropic;
use systemprompt_models::wire::canonical::CanonicalTool;

use crate::error::Result;
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

        let body = anthropic::build_request_body(&canonical, params.model, None);
        let response = post_body(self, &body).await?;

        let stream = response
            .bytes_stream()
            .scan(SseState::default(), |state, item| {
                let out = match item {
                    Ok(bytes) => state.drain(&bytes),
                    Err(e) => vec![Err(crate::error::AiError::Internal(format!(
                        "Stream error: {e}"
                    )))],
                };
                futures::future::ready(Some(out))
            })
            .flat_map(futures::stream::iter);

        Ok(Box::pin(stream))
    }
}

#[derive(Default)]
struct SseState {
    buf: Vec<u8>,
    message_id: String,
}

impl SseState {
    fn drain(&mut self, bytes: &[u8]) -> Vec<Result<StreamChunk>> {
        self.buf.extend_from_slice(bytes);
        let mut chunks = Vec::new();
        while let Some(end) = systemprompt_models::wire::sse::frame_end(&self.buf) {
            let frame: Vec<u8> = self.buf.drain(..end).collect();
            let frame_str = String::from_utf8_lossy(&frame);
            for line in frame_str.lines() {
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                self.capture_message_id(&value);
                if let Some(event) = anthropic::event_from_sse(&value, &self.message_id)
                    && let Some(chunk) = canonical_bridge::event_to_chunk(event)
                {
                    chunks.push(Ok(chunk));
                }
            }
        }
        chunks
    }

    fn capture_message_id(&mut self, value: &Value) {
        if value.get("type").and_then(Value::as_str) == Some("message_start")
            && let Some(id) = value
                .get("message")
                .and_then(|m| m.get("id"))
                .and_then(Value::as_str)
        {
            id.clone_into(&mut self.message_id);
        }
    }
}
