//! Dashboard artifact and its composable sections.
//!
//! A [`DashboardArtifact`] is a titled collection of [`DashboardSection`]s laid
//! out per its [`DashboardHints`]. The submodules supply the section model
//! ([`section`]), per-section data shapes ([`section_data`]), section/layout
//! taxonomy ([`section_types`]), and layout hints ([`hints`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod hints;
pub mod section;
pub mod section_data;
pub mod section_types;

pub use hints::{DashboardHints, LayoutMode};
pub use section::DashboardSection;
pub use section_data::*;
pub use section_types::{LayoutWidth, SectionLayout, SectionType};

use crate::artifacts::metadata::ExecutionMetadata;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use crate::execution::context::RequestContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use systemprompt_identifiers::SkillId;

fn default_artifact_type() -> String {
    "dashboard".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DashboardArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub sections: Vec<DashboardSection>,
    #[serde(skip)]
    #[schemars(skip)]
    hints: DashboardHints,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ExecutionMetadata,
}

impl DashboardArtifact {
    pub const ARTIFACT_TYPE_STR: &'static str = "dashboard";

    pub fn new(title: impl Into<String>) -> Self {
        Self {
            artifact_type: "dashboard".to_owned(),
            title: title.into(),
            description: None,
            sections: Vec::new(),
            hints: DashboardHints::default(),
            metadata: ExecutionMetadata::default(),
        }
    }

    pub fn with_request(mut self, ctx: &RequestContext) -> Self {
        self.metadata = ExecutionMetadata::with_request(ctx);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_sections(mut self, sections: Vec<DashboardSection>) -> Self {
        self.sections = sections;
        self
    }

    pub fn add_section(mut self, section: DashboardSection) -> Self {
        self.sections.push(section);
        self
    }

    pub const fn with_hints(mut self, hints: DashboardHints) -> Self {
        self.hints = hints;
        self
    }

    pub fn with_execution_id(mut self, id: impl Into<String>) -> Self {
        self.metadata.execution_id = Some(id.into());
        self
    }

    pub fn with_skill(
        mut self,
        skill_id: impl Into<SkillId>,
        skill_name: impl Into<String>,
    ) -> Self {
        self.metadata.skill_id = Some(skill_id.into());
        self.metadata.skill_name = Some(skill_name.into());
        self
    }
}

impl Artifact for DashboardArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Dashboard
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Dashboard title"
                },
                "description": {
                    "type": "string",
                    "description": "Dashboard description"
                },
                "sections": {
                    "type": "array",
                    "description": "Dashboard sections",
                    "items": {
                        "type": "object",
                        "properties": {
                            "section_id": {"type": "string"},
                            "title": {"type": "string"},
                            "section_type": {"type": "string"},
                            "data": {"type": "object"},
                            "layout": {"type": "object"}
                        }
                    }
                },
                "_execution_id": {
                    "type": "string",
                    "description": "Execution ID for tracking"
                }
            },
            "required": ["title", "sections"],
            "x-artifact-type": "dashboard",
            "x-dashboard-hints": self.hints.generate_schema()
        })
    }
}
