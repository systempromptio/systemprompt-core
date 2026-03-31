use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContentId, SourceId};

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
