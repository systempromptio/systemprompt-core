use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Bar,
    Line,
    Pie,
    Area,
}

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
    const fn new(data: T, artifact_type: ArtifactType) -> Self {
        Self {
            data,
            artifact_type,
            title: None,
            hints: None,
        }
    }

    pub fn table(data: T) -> Self {
        Self::new(data, ArtifactType::Table)
    }

    pub fn list(data: T) -> Self {
        Self::new(data, ArtifactType::List)
    }

    pub fn card(data: T) -> Self {
        Self::new(data, ArtifactType::PresentationCard)
    }

    pub fn text(data: T) -> Self {
        Self::new(data, ArtifactType::Text)
    }

    pub fn copy_paste(data: T) -> Self {
        Self::new(data, ArtifactType::CopyPasteText)
    }

    pub fn chart(data: T, chart_type: ChartType) -> Self {
        let mut result = Self::new(data, ArtifactType::Chart);
        result.hints = Some(RenderingHints {
            chart_type: Some(chart_type),
            ..Default::default()
        });
        result
    }

    pub fn form(data: T) -> Self {
        Self::new(data, ArtifactType::Form)
    }

    pub fn dashboard(data: T) -> Self {
        Self::new(data, ArtifactType::Dashboard)
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_hints(mut self, hints: RenderingHints) -> Self {
        self.hints = Some(hints);
        self
    }

    pub fn with_columns(mut self, columns: Vec<String>) -> Self {
        let mut hints = self.hints.unwrap_or_default();
        hints.columns = Some(columns);
        self.hints = Some(hints);
        self
    }
}

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
    pub const fn new() -> Self {
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableOutput<T> {
    pub rows: Vec<T>,
}

impl<T> TableOutput<T> {
    pub const fn new(rows: Vec<T>) -> Self {
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

pub fn render_result<T: Serialize>(result: &CommandResult<T>) {
    let config = get_global_config();

    match config.output_format() {
        OutputFormat::Json => {
            CliService::json(result);
        },
        OutputFormat::Yaml => {
            CliService::yaml(result);
        },
        OutputFormat::Table => {
            if let Some(title) = &result.title {
                CliService::section(title);
            }
            CliService::json(&result.data);
        },
    }
}
