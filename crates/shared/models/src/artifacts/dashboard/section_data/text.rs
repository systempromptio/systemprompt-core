//! Free-text and timeline section payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextSectionData {
    pub text: String,
}

impl TextSectionData {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimelineSectionData {
    pub events: Vec<TimelineEvent>,
}

impl TimelineSectionData {
    pub const fn new(events: Vec<TimelineEvent>) -> Self {
        Self { events }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEvent {
    pub timestamp: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl TimelineEvent {
    pub fn new(timestamp: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            timestamp: timestamp.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}
