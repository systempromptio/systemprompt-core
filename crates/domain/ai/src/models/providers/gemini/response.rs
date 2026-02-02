use serde::{Deserialize, Serialize};

use super::request::GeminiContent;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    pub candidates: Vec<GeminiCandidate>,
    pub usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    pub content: Option<GeminiContent>,
    pub finish_reason: Option<String>,
    pub index: Option<i32>,
    pub safety_ratings: Option<Vec<GeminiSafetyRating>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_metadata: Option<GeminiGroundingMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_context_metadata: Option<GeminiUrlContextMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiSafetyRating {
    pub category: String,
    pub probability: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    pub candidates: Option<u32>,
    #[serde(rename = "totalTokenCount")]
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGroundingMetadata {
    #[serde(default)]
    pub grounding_chunks: Vec<GeminiGroundingChunk>,
    #[serde(default)]
    pub grounding_supports: Vec<GeminiGroundingSupport>,
    #[serde(default)]
    pub web_search_queries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiGroundingChunk {
    pub web: GeminiWebSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiWebSource {
    pub uri: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGroundingSupport {
    pub segment: GeminiTextSegment,
    pub grounding_chunk_indices: Vec<i32>,
    #[serde(default)]
    pub confidence_scores: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiTextSegment {
    #[serde(default)]
    pub start_index: i32,
    pub end_index: i32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUrlContextMetadata {
    #[serde(default)]
    pub url_metadata: Vec<GeminiUrlMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUrlMetadata {
    pub retrieved_url: String,
    pub url_retrieval_status: String,
}
