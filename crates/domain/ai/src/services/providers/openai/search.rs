use anyhow::{anyhow, Result};
use std::time::Instant;

use crate::models::ai::{
    AiMessage, MessageRole, SamplingParams, SearchGroundedResponse, WebSource,
};
use crate::models::providers::openai::{
    OpenAiResponsesInput, OpenAiResponsesRequest, OpenAiResponsesResponse, OpenAiResponsesTool,
};

use super::provider::OpenAiProvider;

#[derive(Debug)]
pub struct SearchParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

impl<'a> SearchParams<'a> {
    pub const fn new(messages: &'a [AiMessage], max_output_tokens: u32, model: &'a str) -> Self {
        Self {
            messages,
            sampling: None,
            max_output_tokens,
            model,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }
}

pub async fn generate_with_web_search(
    provider: &OpenAiProvider,
    params: SearchParams<'_>,
) -> Result<SearchGroundedResponse> {
    let start = Instant::now();

    let input: Vec<OpenAiResponsesInput> = params
        .messages
        .iter()
        .map(|msg| OpenAiResponsesInput {
            role: match msg.role {
                MessageRole::User => "user".to_string(),
                MessageRole::Assistant => "assistant".to_string(),
                MessageRole::System => "system".to_string(),
            },
            content: msg.content.clone(),
        })
        .collect();

    let request = OpenAiResponsesRequest {
        model: params.model.to_string(),
        input,
        tools: Some(vec![OpenAiResponsesTool::WebSearch {
            search_context_size: Some("medium".to_string()),
        }]),
        temperature: params.sampling.and_then(|s| s.temperature),
        max_output_tokens: Some(params.max_output_tokens),
    };

    let url = format!("{}/responses", provider.endpoint);

    let response = provider
        .client
        .post(&url)
        .header("Authorization", format!("Bearer {}", &provider.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading response: {}>", e));
        return Err(anyhow!(
            "OpenAI API returned status {}: {}",
            status,
            error_body
        ));
    }

    let responses_response: OpenAiResponsesResponse = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

    Ok(extract_search_response(&responses_response, start))
}

fn extract_search_response(
    response: &OpenAiResponsesResponse,
    start: Instant,
) -> SearchGroundedResponse {
    let mut content_text = String::new();
    let mut sources = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for output in &response.output {
        if output.r#type == "message" {
            if let Some(contents) = &output.content {
                for content in contents {
                    if content.r#type == "output_text" {
                        if let Some(text) = &content.text {
                            content_text.push_str(text);
                        }
                    }

                    if let Some(annotations) = &content.annotations {
                        for annotation in annotations {
                            if annotation.r#type == "url_citation" {
                                if let (Some(url), Some(title)) =
                                    (&annotation.url, &annotation.title)
                                {
                                    if seen_urls.insert(url.clone()) {
                                        sources.push(WebSource {
                                            title: title.clone(),
                                            uri: url.clone(),
                                            relevance: 1.0,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let latency_ms = start.elapsed().as_millis() as u64;

    SearchGroundedResponse {
        content: content_text,
        sources,
        confidence_scores: Vec::new(),
        web_search_queries: Vec::new(),
        url_context_metadata: None,
        tokens_used: response.usage.as_ref().map(|u| u.total_tokens),
        latency_ms,
        finish_reason: Some("stop".to_string()),
        safety_ratings: None,
    }
}
