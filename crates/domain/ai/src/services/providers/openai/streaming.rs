use anyhow::{anyhow, Result};
use futures::{Stream, StreamExt};
use serde_json::json;
use std::pin::Pin;

use crate::models::ai::{AiMessage, SamplingParams};
use crate::models::providers::openai::OpenAiTool;

use super::provider::OpenAiProvider;
use super::reasoning;

pub struct StreamRequestParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub tools: Option<Vec<OpenAiTool>>,
}

impl OpenAiProvider {
    pub(crate) async fn create_stream_request(
        &self,
        params: StreamRequestParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let openai_messages: Vec<crate::models::providers::openai::OpenAiMessage> =
            params.messages.iter().map(Into::into).collect();

        let temperature = params.sampling.and_then(|s| s.temperature).unwrap_or(0.8);

        let mut request_body = json!({
            "model": params.model,
            "messages": openai_messages,
            "temperature": temperature,
            "max_tokens": params.max_output_tokens,
            "stream": true
        });

        if let Some(tools) = params.tools {
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

        let stream = response.bytes_stream().map(|chunk| -> Result<String> {
            match chunk {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let mut content_parts = Vec::new();

                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }

                            if let Ok(chunk_json) = serde_json::from_str::<serde_json::Value>(data)
                            {
                                if let Some(choices) = chunk_json["choices"].as_array() {
                                    if let Some(first_choice) = choices.first() {
                                        if let Some(content) =
                                            first_choice["delta"]["content"].as_str()
                                        {
                                            content_parts.push(content.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Ok(content_parts.join(""))
                },
                Err(e) => Err(anyhow!("Stream error: {e}")),
            }
        });

        Ok(Box::pin(stream))
    }
}
