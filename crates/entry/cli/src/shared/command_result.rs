//! Command result types for CLI output
//!
//! All CLI commands should return `CommandResult<T>` where T is a type that
//! derives `Serialize, Deserialize, JsonSchema`. The result includes metadata
//! about how to render the output.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The type of artifact to render
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Table,
    List,
    PresentationCard,
    Text,
    CopyPasteText,
    Chart,
    Form,
    Dashboard,
}

/// Chart type for Chart artifacts
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Bar,
    Line,
    Pie,
    Area,
}

/// Rendering hints for artifact display
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RenderingHints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_type: Option<ChartType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A command result that wraps output data with metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommandResult<T> {
    pub data: T,
    pub artifact_type: ArtifactType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<RenderingHints>,
}

impl<T> CommandResult<T> {
    /// Create a new command result with the given artifact type
    fn new(data: T, artifact_type: ArtifactType) -> Self {
        Self {
            data,
            artifact_type,
            title: None,
            hints: None,
        }
    }

    /// Create a table artifact for multi-row data
    pub fn table(data: T) -> Self {
        Self::new(data, ArtifactType::Table)
    }

    /// Create a list artifact for simple item arrays
    pub fn list(data: T) -> Self {
        Self::new(data, ArtifactType::List)
    }

    /// Create a presentation card artifact for single entity detail
    pub fn card(data: T) -> Self {
        Self::new(data, ArtifactType::PresentationCard)
    }

    /// Create a text artifact for plain text messages
    pub fn text(data: T) -> Self {
        Self::new(data, ArtifactType::Text)
    }

    /// Create a copy-paste text artifact for tokens/keys
    pub fn copy_paste(data: T) -> Self {
        Self::new(data, ArtifactType::CopyPasteText)
    }

    /// Create a chart artifact for metrics/analytics
    pub fn chart(data: T, chart_type: ChartType) -> Self {
        let mut result = Self::new(data, ArtifactType::Chart);
        result.hints = Some(RenderingHints {
            chart_type: Some(chart_type),
            ..Default::default()
        });
        result
    }

    /// Create a form artifact for configuration view
    pub fn form(data: T) -> Self {
        Self::new(data, ArtifactType::Form)
    }

    /// Create a dashboard artifact for multi-panel view
    pub fn dashboard(data: T) -> Self {
        Self::new(data, ArtifactType::Dashboard)
    }

    /// Add a title for human display
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add rendering hints
    pub fn with_hints(mut self, hints: RenderingHints) -> Self {
        self.hints = Some(hints);
        self
    }

    /// Add column hints for table rendering
    pub fn with_columns(mut self, columns: Vec<String>) -> Self {
        let mut hints = self.hints.unwrap_or_default();
        hints.columns = Some(columns);
        self.hints = Some(hints);
        self
    }
}

// Common output types

/// Simple text output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextOutput {
    pub message: String,
}

impl TextOutput {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Success message output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SuccessOutput {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<String>>,
}

impl SuccessOutput {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = Some(details);
        self
    }
}

/// Key-value pair output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KeyValueOutput {
    pub items: Vec<KeyValueItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KeyValueItem {
    pub key: String,
    pub value: String,
}

impl KeyValueOutput {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.items.push(KeyValueItem {
            key: key.into(),
            value: value.into(),
        });
        self
    }
}

impl Default for KeyValueOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// Table row for generic table output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableOutput<T> {
    pub rows: Vec<T>,
}

impl<T> TableOutput<T> {
    pub fn new(rows: Vec<T>) -> Self {
        Self { rows }
    }
}

impl<T> Default for TableOutput<T> {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}

use crate::cli_settings::{get_global_config, OutputFormat};
use systemprompt_core_logging::CliService;

/// Render a command result based on the current output format
pub fn render_result<T: Serialize>(result: &CommandResult<T>) {
    let config = get_global_config();

    match config.output_format() {
        OutputFormat::Json => {
            CliService::json(result);
        }
        OutputFormat::Yaml => {
            CliService::yaml(result);
        }
        OutputFormat::Table => {
            // For table format, we render the title and then the data
            if let Some(title) = &result.title {
                CliService::section(title);
            }
            // The actual rendering is type-specific, so we just output JSON for now
            // Individual commands can implement custom table rendering
            CliService::json(&result.data);
        }
    }
}
