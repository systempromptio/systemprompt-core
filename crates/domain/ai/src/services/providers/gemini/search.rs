use anyhow::{anyhow, Result};
use std::time::Instant;

use crate::models::ai::{AiMessage, SamplingParams, SearchGroundedResponse, WebSource};
use crate::models::providers::gemini::{
    GeminiPart, GeminiRequest, GeminiResponse, GeminiTool, GoogleSearch, UrlContext,
};

use super::constants::defaults;
use super::provider::GeminiProvider;
use super::{converters, request_builders};

pub struct SearchParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub urls: Option<Vec<String>>,
}

pub struct SearchParamsBuilder<'a> {
    messages: &'a [AiMessage],
    sampling: Option<&'a SamplingParams>,
    max_output_tokens: u32,
    model: &'a str,
    urls: Option<Vec<String>>,
}

impl<'a> SearchParamsBuilder<'a> {
    pub const fn new(messages: &'a [AiMessage], max_output_tokens: u32, model: &'a str) -> Self {
        Self {
            messages,
            sampling: None,
            max_output_tokens,
            model,
            urls: None,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub fn with_urls(mut self, urls: Vec<String>) -> Self {
        self.urls = Some(urls);
        self
    }

    pub fn build(self) -> SearchParams<'a> {
        SearchParams {
            messages: self.messages,
            sampling: self.sampling,
            max_output_tokens: self.max_output_tokens,
            model: self.model,
            urls: self.urls,
        }
    }
}

impl<'a> SearchParams<'a> {
    pub const fn builder(
        messages: &'a [AiMessage],
        max_output_tokens: u32,
        model: &'a str,
    ) -> SearchParamsBuilder<'a> {
        SearchParamsBuilder::new(messages, max_output_tokens, model)
    }
}

pub async fn generate_with_google_search(
    provider: &GeminiProvider,
    params: SearchParams<'_>,
) -> Result<SearchGroundedResponse> {
    let start = Instant::now();

    let contents = converters::convert_messages(params.messages);
    let generation_config = request_builders::build_generation_config(
        params.sampling,
        params.max_output_tokens,
        None,
        None,
    );

    let gemini_tools = build_search_tools(params.urls.is_some());

    let request = GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: Some(gemini_tools),
        tool_config: None,
    };

    let response_text =
        request_builders::send_request(provider, &request, params.model, "generateContent").await?;

    let gemini_response: GeminiResponse = request_builders::parse_response(&response_text)?;

    extract_grounded_response(&gemini_response, start)
}

fn build_search_tools(include_url_context: bool) -> Vec<GeminiTool> {
    let mut tools = vec![GeminiTool {
        function_declarations: None,
        google_search: Some(GoogleSearch {}),
        url_context: None,
        code_execution: None,
    }];

    if include_url_context {
        tools.push(GeminiTool {
            function_declarations: None,
            google_search: None,
            url_context: Some(UrlContext {}),
            code_execution: None,
        });
    }

    tools
}

fn extract_grounded_response(
    response: &GeminiResponse,
    start: Instant,
) -> Result<SearchGroundedResponse> {
    let candidate = response
        .candidates
        .first()
        .ok_or_else(|| anyhow!("No response from Gemini"))?;

    let content_text = candidate
        .content
        .as_ref()
        .and_then(|c| {
            c.parts.iter().find_map(|p| match p {
                GeminiPart::Text { text } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();

    let mut sources = Vec::new();
    let mut confidence_scores = Vec::new();
    let mut web_search_queries = Vec::new();

    if let Some(grounding) = &candidate.grounding_metadata {
        for chunk in &grounding.grounding_chunks {
            sources.push(WebSource {
                title: chunk.web.title.clone(),
                uri: chunk.web.uri.clone(),
                relevance: defaults::RELEVANCE_SCORE,
            });
        }

        for support in &grounding.grounding_supports {
            for score in &support.confidence_scores {
                confidence_scores.push(*score);
            }
        }

        web_search_queries.clone_from(&grounding.web_search_queries);
    }

    let url_context_metadata = candidate.url_context_metadata.as_ref().map(|meta| {
        use systemprompt_models::ai::UrlMetadata;
        meta.url_metadata
            .iter()
            .map(|url_meta| UrlMetadata {
                retrieved_url: url_meta.retrieved_url.clone(),
                url_retrieval_status: url_meta.url_retrieval_status.clone(),
            })
            .collect()
    });

    let latency_ms = start.elapsed().as_millis() as u64;

    let finish_reason = candidate.finish_reason.clone();
    let safety_ratings = candidate.safety_ratings.as_ref().map(|ratings| {
        ratings
            .iter()
            .map(|r| {
                serde_json::json!({
                    "category": r.category,
                    "probability": r.probability
                })
            })
            .collect()
    });

    Ok(SearchGroundedResponse {
        content: content_text,
        sources,
        confidence_scores,
        web_search_queries,
        url_context_metadata,
        tokens_used: response.usage_metadata.as_ref().map(|u| u.total),
        latency_ms,
        finish_reason,
        safety_ratings,
    })
}
