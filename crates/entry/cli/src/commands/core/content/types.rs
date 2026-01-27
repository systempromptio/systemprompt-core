use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    CampaignId, CategoryId, ContentId, LinkId, SessionId, SourceId, UserId,
};

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

const fn is_zero(val: &usize) -> bool {
    *val == 0
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
pub struct GenerateLinkOutput {
    pub link_id: LinkId,
    pub short_code: String,
    pub short_url: String,
    pub target_url: String,
    pub full_url: String,
    pub link_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utm_params: Option<UtmParamsOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UtmParamsOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LinkDetailOutput {
    pub id: LinkId,
    pub short_code: String,
    pub target_url: String,
    pub full_url: String,
    pub link_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<CampaignId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_content_id: Option<ContentId>,
    pub click_count: i32,
    pub unique_click_count: i32,
    pub conversion_count: i32,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LinkListOutput {
    pub links: Vec<LinkSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LinkSummary {
    pub id: LinkId,
    pub short_code: String,
    pub target_url: String,
    pub link_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_name: Option<String>,
    pub click_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LinkPerformanceOutput {
    pub link_id: LinkId,
    pub click_count: i64,
    pub unique_click_count: i64,
    pub conversion_count: i64,
    pub conversion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClicksOutput {
    pub link_id: LinkId,
    pub clicks: Vec<ClickRow>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClickRow {
    pub click_id: String,
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    pub clicked_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer_page: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub is_conversion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CampaignAnalyticsOutput {
    pub campaign_id: CampaignId,
    pub total_clicks: i64,
    pub link_count: i64,
    pub unique_visitors: i64,
    pub conversion_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JourneyOutput {
    pub nodes: Vec<JourneyNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JourneyNode {
    pub source_content_id: ContentId,
    pub target_url: String,
    pub click_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LinkDeleteOutput {
    pub deleted: bool,
    pub link_id: LinkId,
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
pub struct PublishOutput {
    pub content_id: ContentId,
    pub slug: String,
    pub source_id: SourceId,
    pub action: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerendered: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateOutput {
    pub content_id: ContentId,
    pub slug: String,
    pub updated_fields: Vec<String>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportOutput {
    pub exported_count: i64,
    pub output_directory: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnhancedIngestOutput {
    pub files_found: usize,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub errors: Vec<String>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PublishPipelineOutput {
    pub steps: Vec<StepResult>,
    pub total_steps: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StepError {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub struct StepErrorBuilder {
    summary: String,
    cause: Option<String>,
    location: Option<String>,
    suggestion: Option<String>,
}

impl StepErrorBuilder {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            cause: None,
            location: None,
            suggestion: None,
        }
    }

    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn build(self) -> StepError {
        StepError {
            summary: self.summary,
            cause: self.cause,
            location: self.location,
            suggestion: self.suggestion,
        }
    }
}

impl StepError {
    pub fn builder(summary: impl Into<String>) -> StepErrorBuilder {
        StepErrorBuilder::new(summary)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StepResult {
    pub step: String,
    pub success: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StepError>,
}
