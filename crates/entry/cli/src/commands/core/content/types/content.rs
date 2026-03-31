use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentListOutput {
    pub items: Vec<ContentSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentSummary {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub kind: String,
    pub source_id: SourceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<CategoryId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentDetailOutput {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<Utc>>,
    pub keywords: Vec<String>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<CategoryId>,
    pub source_id: SourceId,
    pub version_hash: String,
    pub is_public: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchOutput {
    pub results: Vec<SearchResultRow>,
    pub total: i64,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResultRow {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub source_id: SourceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<CategoryId>,
}

pub const fn is_zero(val: &usize) -> bool {
    *val == 0
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IngestOutput {
    pub files_found: usize,
    pub files_processed: usize,
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_create: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_update: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unchanged_count: usize,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AllSourcesIngestOutput {
    pub sources_processed: usize,
    pub total_files_found: usize,
    pub total_files_processed: usize,
    pub source_results: Vec<SourceIngestResult>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SourceIngestResult {
    pub source_id: SourceId,
    pub files_found: usize,
    pub files_processed: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_create: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_update: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unchanged_count: usize,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteOutput {
    pub deleted: bool,
    pub content_id: ContentId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteSourceOutput {
    pub deleted_count: u64,
    pub source_id: SourceId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PopularOutput {
    pub items: Vec<ContentSummary>,
    pub source_id: SourceId,
    pub days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerifyOutput {
    pub content_id: ContentId,
    pub slug: String,
    pub source_id: SourceId,
    pub in_database: bool,
    pub is_public: bool,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerendered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerender_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusOutput {
    pub items: Vec<ContentStatusRow>,
    pub source_id: SourceId,
    pub total: i64,
    pub healthy: i64,
    pub issues: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentStatusRow {
    pub slug: String,
    pub title: String,
    pub in_database: bool,
    pub is_public: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerendered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateOutput {
    pub content_id: ContentId,
    pub slug: String,
    pub updated_fields: Vec<String>,
    pub success: bool,
}
