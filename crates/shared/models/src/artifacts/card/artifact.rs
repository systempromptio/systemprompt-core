//! The presentation-card artifact type and its [`Artifact`] implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use systemprompt_identifiers::SkillId;

use super::{CardCta, CardSection, CardTheme};
use crate::artifacts::metadata::ExecutionMetadata;
use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;
use crate::execution::context::RequestContext;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresentationCardArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_card_artifact_type")]
    pub artifact_type: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub sections: Vec<CardSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ctas: Vec<CardCta>,
    #[serde(default)]
    pub theme: CardTheme,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<SkillId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    #[serde(skip)]
    #[schemars(skip)]
    metadata: ExecutionMetadata,
}

fn default_card_artifact_type() -> String {
    "presentation_card".to_owned()
}

impl PresentationCardArtifact {
    pub const ARTIFACT_TYPE_STR: &'static str = "presentation_card";

    pub fn new(title: impl Into<String>) -> Self {
        Self {
            artifact_type: "presentation_card".to_owned(),
            title: title.into(),
            subtitle: None,
            sections: Vec::new(),
            ctas: Vec::new(),
            theme: CardTheme::default(),
            execution_id: None,
            skill_id: None,
            skill_name: None,
            metadata: ExecutionMetadata::default(),
        }
    }

    pub fn with_request(mut self, ctx: &RequestContext) -> Self {
        self.metadata = ExecutionMetadata::with_request(ctx);
        self
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_sections(mut self, sections: Vec<CardSection>) -> Self {
        self.sections = sections;
        self
    }

    pub fn add_section(mut self, section: CardSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn with_ctas(mut self, ctas: Vec<CardCta>) -> Self {
        self.ctas = ctas;
        self
    }

    pub fn add_cta(mut self, cta: CardCta) -> Self {
        self.ctas.push(cta);
        self
    }

    #[must_use]
    pub const fn with_theme(mut self, theme: CardTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_execution_id(mut self, id: impl Into<String>) -> Self {
        let id_str = id.into();
        self.execution_id = Some(id_str.clone());
        self.metadata.execution_id = Some(id_str);
        self
    }

    pub fn with_skill(
        mut self,
        skill_id: impl Into<SkillId>,
        skill_name: impl Into<String>,
    ) -> Self {
        let id = skill_id.into();
        self.skill_id = Some(id.clone());
        self.skill_name = Some(skill_name.into());
        self.metadata.skill_id = Some(id);
        self
    }
}

impl Artifact for PresentationCardArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::PresentationCard
    }

    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Card title"
                },
                "subtitle": {
                    "type": "string",
                    "description": "Card subtitle"
                },
                "sections": {
                    "type": "array",
                    "description": "Content sections",
                    "items": {
                        "type": "object",
                        "properties": {
                            "heading": {"type": "string"},
                            "content": {"description": "Section content: plain string or structured JSON"},
                            "icon": {"type": "string"}
                        },
                        "required": ["heading", "content"]
                    }
                },
                "ctas": {
                    "type": "array",
                    "description": "Call-to-action buttons",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "label": {"type": "string"},
                            "message": {"type": "string"},
                            "variant": {"type": "string", "enum": ["primary", "secondary", "danger"]},
                            "icon": {"type": "string"}
                        },
                        "required": ["id", "label", "message", "variant"]
                    }
                },
                "theme": {
                    "type": "string",
                    "enum": ["gradient", "plain", "muted"],
                    "description": "Card theme",
                    "default": "gradient"
                },
                "_execution_id": {
                    "type": "string",
                    "description": "Execution ID for tracking"
                }
            },
            "required": ["title", "sections"],
            "x-artifact-type": "presentation_card",
            "x-presentation-hints": {
                "theme": self.theme
            }
        })
    }
}
