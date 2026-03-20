use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use serde_json::json;
use std::pin::Pin;

use crate::models::providers::openai::{OpenAiStreamChunk, OpenAiTool};
use crate::services::providers::GenerationParams;
use systemprompt_models::ai::StreamChunk;

use super::provider::OpenAiProvider;
use super::reasoning;

impl OpenAiProvider {
    pub(crate) async fn create_stream_request(
        &self,
        params: GenerationParams<'_>,
        tools: Option<Vec<OpenAiTool>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let openai_messages: Vec<crate::models::providers::openai::OpenAiMessage> =
            params.messages.iter().map(Into::into).collect();

        let temperature = params.sampling.and_then(|s| s.temperature).unwrap_or(0.8);

        let mut request_body = json!({
            "model": params.model,
            "messages": openai_messages,
            "temperature": temperature,
            "max_tokens": params.max_output_tokens,
            "stream": true,
            "stream_options": {"include_usage": true}
        });

        if let Some(tools) = tools {
            request_body["tools"] = json!(tools);
        }

        if let Some(reasoning_effort) = reasoning::build_reasoning_config(params.model) {
            request_body["reasoning_effort"] = json!(reasoning_effort);
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.endpoint))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow!("OpenAI API error ({status}): {error_text}"));
        }

        let stream = response
            .bytes_stream()
            .map(|chunk| -> Result<Vec<StreamChunk>> {
                match chunk {
                    Ok(bytes) => Ok(parse_openai_sse_chunks(&bytes)),
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

fn parse_openai_sse_chunks(bytes: &bytes::Bytes) -> Vec<StreamChunk> {
    let text = String::from_utf8_lossy(bytes);
    let mut chunks = Vec::new();

    for line in text.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };

        if data == "[DONE]" {
            continue;
        }

        let Ok(parsed) = serde_json::from_str::<OpenAiStreamChunk>(data) else {
            continue;
        };

        if let Some(choice) = parsed.choices.first() {
            if let Some(content) = &choice.delta.content {
                if !content.is_empty() {
                    chunks.push(StreamChunk::Text(content.clone()));
                }
            }
        }

        if let Some(usage) = parsed.usage {
            let cached = usage.prompt_tokens_details.and_then(|d| d.cached_tokens);
            chunks.push(StreamChunk::Usage {
                input_tokens: Some(usage.prompt_tokens),
                output_tokens: Some(usage.completion_tokens),
                tokens_used: Some(usage.total_tokens),
                cache_read_tokens: cached,
                cache_creation_tokens: None,
                finish_reason: None,
            });
        }
    }

    chunks
}
