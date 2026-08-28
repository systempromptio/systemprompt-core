//! Presentation-card artifact.
//!
//! A [`PresentationCardArtifact`] renders a titled card composed of
//! [`CardSection`]s and optional [`CardCta`] action buttons under a named
//! theme. [`PresentationCardResponse`] is the matching deserialization shape
//! for tool output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod artifact;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::SkillId;

pub use artifact::PresentationCardArtifact;

/// A card's visual treatment.
///
/// This was a free `String` interpolated straight into a `card-theme-{}` class
/// name, so any value the stylesheet did not happen to define produced a class
/// with no rules and a silently unstyled card. The renderer now cannot be
/// handed a value it has no treatment for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum CardTheme {
    #[default]
    Gradient,
    Plain,
    Muted,
    #[serde(other)]
    Unknown,
}

impl CardTheme {
    #[must_use]
    pub const fn class_suffix(self) -> &'static str {
        match self {
            Self::Gradient | Self::Unknown => "gradient",
            Self::Plain => "plain",
            Self::Muted => "muted",
        }
    }
}

/// A CTA button's visual weight.
///
/// Same defect as [`CardTheme`]: `"secondary"` was accepted, rendered as
/// `card-cta-secondary`, and had no rule — which is why the email draft's
/// Discard button came out unstyled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum CtaVariant {
    #[default]
    Primary,
    Secondary,
    Danger,
    #[serde(other)]
    Unknown,
}

impl CtaVariant {
    #[must_use]
    pub const fn class_suffix(self) -> &'static str {
        match self {
            Self::Primary | Self::Unknown => "primary",
            Self::Secondary => "secondary",
            Self::Danger => "danger",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PresentationCardResponse {
    #[serde(rename = "x-artifact-type")]
    pub artifact_type: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub sections: Vec<CardSection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ctas: Vec<CardCta>,
    pub theme: CardTheme,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<SkillId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CardSection {
    pub heading: String,
    pub content: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl CardSection {
    pub fn new(heading: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
            content: JsonValue::String(content.into()),
            icon: None,
        }
    }

    #[must_use]
    pub fn value(heading: impl Into<String>, content: JsonValue) -> Self {
        Self {
            heading: heading.into(),
            content,
            icon: None,
        }
    }

    #[must_use]
    pub fn content_display(&self) -> String {
        match &self.content {
            JsonValue::String(s) => s.clone(),
            JsonValue::Null => String::new(),
            JsonValue::Array(items) => items
                .iter()
                .map(Self::scalar_text)
                .collect::<Vec<_>>()
                .join(", "),
            other => Self::scalar_text(other),
        }
    }

    fn scalar_text(value: &JsonValue) -> String {
        match value {
            JsonValue::String(s) => s.clone(),
            JsonValue::Null => String::new(),
            other => other.to_string(),
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CardCta {
    pub id: String,
    pub label: String,
    pub message: String,
    pub variant: CtaVariant,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl CardCta {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        message: impl Into<String>,
        variant: CtaVariant,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            message: message.into(),
            variant,
            icon: None,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}
