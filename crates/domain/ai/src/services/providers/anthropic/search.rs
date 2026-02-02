use anyhow::{anyhow, Result};
use std::time::Instant;

use crate::models::ai::{AiMessage, SamplingParams, SearchGroundedResponse, WebSource};
use crate::models::providers::anthropic::{
    AnthropicSearchContentBlock, AnthropicSearchRequest, AnthropicSearchResponse,
    AnthropicServerTool, AnthropicWebSearchResultItem,
};

use super::converters;
use super::provider::AnthropicProvider;

#[derive(Debug)]
pub struct SearchParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub max_uses: Option<u32>,
}

impl<'a> SearchParams<'a> {
    pub const fn new(messages: &'a [AiMessage], max_output_tokens: u32, model: &'a str) -> Self {
        Self {
            messages,
            sampling: None,
            max_output_tokens,
            model,
            max_uses: None,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub const fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }
}

pub async fn generate_with_web_search(
    provider: &AnthropicProvider,
    params: SearchParams<'_>,
) -> Result<SearchGroundedResponse> {
    let start = Instant::now();

    let (system_prompt, anthropic_messages) = converters::convert_messages(params.messages);

    let (temperature, top_p, top_k) = params
        .sampling
        .map_or((None, None, None), |s| (s.temperature, s.top_p, s.top_k));

    let web_search_tool = AnthropicServerTool::WebSearch {
        name: "web_search".to_string(),
        max_uses: params.max_uses.or(Some(5)),
    };

    let request = AnthropicSearchRequest {
        model: params.model.to_string(),
        messages: anthropic_messages,
        max_tokens: params.max_output_tokens,
        temperature,
        top_p,
        top_k,
        system: system_prompt,
        tools: vec![web_search_tool],
    };

    let response = provider
        .client
        .post(format!("{}/messages", provider.endpoint))
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
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
            "Anthropic API returned status {}: {}",
            status,
            error_body
        ));
    }

    let search_response: AnthropicSearchResponse = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

    Ok(extract_search_response(&search_response, start))
}

fn extract_search_response(
    response: &AnthropicSearchResponse,
    start: Instant,
) -> SearchGroundedResponse {
    let mut content_text = String::new();
    let mut sources = Vec::new();
    let mut web_search_queries = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for block in &response.content {
        match block {
            AnthropicSearchContentBlock::Text { text, citations } => {
                content_text.push_str(text);

                if let Some(cites) = citations {
                    for citation in cites {
                        if seen_urls.insert(citation.url.clone()) {
                            sources.push(WebSource {
                                title: citation.title.clone(),
                                uri: citation.url.clone(),
                                relevance: 1.0,
                            });
                        }
                    }
                }
            },
            AnthropicSearchContentBlock::ServerToolUse { input, .. } => {
                if let Some(query) = input.get("query").and_then(|q| q.as_str()) {
                    web_search_queries.push(query.to_string());
                }
            },
            AnthropicSearchContentBlock::WebSearchToolResult { content, .. } => {
                for item in content {
                    if let AnthropicWebSearchResultItem::WebSearchResult { url, title, .. } = item {
                        if seen_urls.insert(url.clone()) {
                            sources.push(WebSource {
                                title: title.clone(),
                                uri: url.clone(),
                                relevance: 1.0,
                            });
                        }
                    }
                }
            },
        }
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    let tokens_used = Some(response.usage.input_tokens + response.usage.output_tokens);

    SearchGroundedResponse {
        content: content_text,
        sources,
        confidence_scores: Vec::new(),
        web_search_queries,
        url_context_metadata: None,
        tokens_used,
        latency_ms,
        finish_reason: response.stop_reason.clone(),
        safety_ratings: None,
    }
}
