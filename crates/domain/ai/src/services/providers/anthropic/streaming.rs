use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use std::pin::Pin;

use crate::models::providers::anthropic::{
    AnthropicDelta, AnthropicRequest, AnthropicStreamEvent, AnthropicTool,
};
use crate::services::providers::GenerationParams;
use systemprompt_models::ai::StreamChunk;

use super::provider::AnthropicProvider;
use super::{converters, thinking};

impl AnthropicProvider {
    pub(crate) async fn create_stream_request(
        &self,
        params: GenerationParams<'_>,
        tools: Option<Vec<AnthropicTool>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let (system_prompt, anthropic_messages) = converters::convert_messages(params.messages);

        let (temperature, top_p, top_k, stop_sequences) =
            params.sampling.map_or((None, None, None, None), |s| {
                (s.temperature, s.top_p, s.top_k, s.stop_sequences.clone())
            });

        let thinking_config = thinking::build_thinking_config(params.model);

        let request = AnthropicRequest {
            model: params.model.to_string(),
            messages: anthropic_messages,
            max_tokens: params.max_output_tokens,
            temperature,
            top_p,
            top_k,
            stop_sequences,
            system: system_prompt,
            tools,
            tool_choice: None,
            stream: Some(true),
            thinking: thinking_config,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.endpoint))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow!(
                "Anthropic streaming API error ({status}): {error_text}"
            ));
        }

        let stream = response
            .bytes_stream()
            .map(|chunk| -> Result<Vec<StreamChunk>> {
                match chunk {
                    Ok(bytes) => Ok(parse_sse_chunks(&bytes)),
                    Err(e) => Err(anyhow!("Stream error: {e}")),
                }
            })
            .flat_map(|result| match result {
                Ok(chunks) => futures::stream::iter(chunks.into_iter().map(Ok)).boxed(),
                Err(e) => futures::stream::iter(vec![Err(e)]).boxed(),
            });

        Ok(Box::pin(stream))
    }
}

fn parse_sse_chunks(bytes: &bytes::Bytes) -> Vec<StreamChunk> {
    let text = String::from_utf8_lossy(bytes);
    let mut chunks = Vec::new();

    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                chunks.extend(extract_chunks_from_event(&event));
            }
        }
    }

    chunks
}

fn extract_chunks_from_event(event: &AnthropicStreamEvent) -> Vec<StreamChunk> {
    match event {
        AnthropicStreamEvent::ContentBlockDelta { delta, .. } => match delta {
            AnthropicDelta::TextDelta { text } => vec![StreamChunk::Text(text.clone())],
            AnthropicDelta::InputJsonDelta { .. } => vec![],
        },
        AnthropicStreamEvent::MessageStart { message } => {
            vec![StreamChunk::Usage {
                input_tokens: Some(message.usage.input),
                output_tokens: Some(message.usage.output),
                tokens_used: Some(message.usage.input + message.usage.output),
                cache_read_tokens: message.usage.cache_read,
                cache_creation_tokens: message.usage.cache_creation,
                finish_reason: None,
            }]
        },
        AnthropicStreamEvent::MessageDelta { delta, usage } => {
            vec![StreamChunk::Usage {
                input_tokens: None,
                output_tokens: Some(usage.output_tokens),
                tokens_used: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                finish_reason: delta.stop_reason.clone(),
            }]
        },
        _ => vec![],
    }
}
