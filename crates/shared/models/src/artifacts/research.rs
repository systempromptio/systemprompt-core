use crate::artifacts::card::PresentationCardResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResearchArtifact {
    #[serde(flatten)]
    pub card: PresentationCardResponse,
    pub topic: String,
    pub sources: Vec<SourceCitation>,
    pub query_count: u32,
    pub source_count: u32,
}

impl ResearchArtifact {
    pub const ARTIFACT_TYPE: &'static str = "presentation_card";

    pub fn new(
        topic: impl Into<String>,
        card: PresentationCardResponse,
        sources: Vec<SourceCitation>,
    ) -> Self {
        let source_count = u32::try_from(sources.len()).unwrap_or(u32::MAX);
        Self {
            card,
            topic: topic.into(),
            sources,
            query_count: 0,
            source_count,
        }
    }

    pub const fn with_query_count(mut self, count: u32) -> Self {
        self.query_count = count;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SourceCitation {
    pub title: String,
    pub uri: String,
    pub relevance: f32,
}

impl SourceCitation {
    pub fn new(title: impl Into<String>, uri: impl Into<String>, relevance: f32) -> Self {
        Self {
            title: title.into(),
            uri: uri.into(),
            relevance,
        }
    }
}
