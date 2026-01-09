use anyhow::{anyhow, Result};
use futures::{Stream, StreamExt};
use std::pin::Pin;

use crate::models::ai::{AiMessage, SamplingParams};
use crate::models::providers::anthropic::{
    AnthropicDelta, AnthropicRequest, AnthropicStreamEvent, AnthropicTool,
};

use super::provider::AnthropicProvider;
use super::{converters, thinking};

pub struct StreamRequestParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub tools: Option<Vec<AnthropicTool>>,
}

impl AnthropicProvider {
    pub(crate) async fn create_stream_request(
        &self,
        params: StreamRequestParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
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
            tools: params.tools,
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

        let stream = response.bytes_stream().map(|chunk| -> Result<String> {
            match chunk {
                Ok(bytes) => Ok(parse_sse_chunk(&bytes)),
                Err(e) => Err(anyhow!("Stream error: {e}")),
            }
        });

        Ok(Box::pin(stream))
    }
}

fn parse_sse_chunk(bytes: &bytes::Bytes) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut content_parts = Vec::new();

    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                if let Some(text) = extract_text_from_event(&event) {
                    content_parts.push(text);
                }
            }
        }
    }

    content_parts.join("")
}

fn extract_text_from_event(event: &AnthropicStreamEvent) -> Option<String> {
    match event {
        AnthropicStreamEvent::ContentBlockDelta { delta, .. } => match delta {
            AnthropicDelta::TextDelta { text } => Some(text.clone()),
            AnthropicDelta::InputJsonDelta { .. } => None,
        },
        _ => None,
    }
}
