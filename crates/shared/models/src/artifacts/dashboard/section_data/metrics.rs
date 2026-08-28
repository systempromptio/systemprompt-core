//! Metric-card section payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MetricsCardsData {
    pub cards: Vec<MetricCard>,
}

impl MetricsCardsData {
    pub const fn new(cards: Vec<MetricCard>) -> Self {
        Self { cards }
    }

    pub fn add_card(mut self, card: MetricCard) -> Self {
        self.cards.push(card);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MetricCard {
    pub title: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MetricStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum MetricStatus {
    Success,
    Warning,
    Error,
    #[default]
    Info,
}

impl std::str::FromStr for MetricStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "success" | "healthy" | "ok" | "active" => Ok(Self::Success),
            "warning" | "degraded" => Ok(Self::Warning),
            "error" | "failed" | "critical" => Ok(Self::Error),
            "info" | "unknown" => Ok(Self::Info),
            _ => Err(format!("Invalid metric status: {s}")),
        }
    }
}

impl MetricCard {
    pub fn new(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            subtitle: None,
            icon: None,
            status: None,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub const fn with_status(mut self, status: MetricStatus) -> Self {
        self.status = Some(status);
        self
    }
}
