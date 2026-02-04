//! CLI artifact wrapper for MCP tool responses.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use thiserror::Error;

use super::list::ListItem;
use super::table::Column;
use super::types::ColumnType;
use super::{CopyPasteTextArtifact, DashboardArtifact, ListArtifact, TableArtifact, TextArtifact};
use crate::execution::context::RequestContext;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Missing columns hint for table artifact")]
    MissingColumns,

    #[error("No array found in data for table/list conversion")]
    NoArrayFound,

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unsupported artifact type: {0}")]
    UnsupportedType(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CliArtifactType {
    Table,
    List,
    PresentationCard,
    Text,
    CopyPasteText,
    Chart,
    Form,
    Dashboard,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RenderingHints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommandResultRaw {
    pub data: JsonValue,
    pub artifact_type: CliArtifactType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<RenderingHints>,
}

impl CommandResultRaw {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn from_value(value: JsonValue) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    pub fn to_cli_artifact(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError> {
        match self.artifact_type {
            CliArtifactType::Table => self.convert_table(ctx),
            CliArtifactType::List => self.convert_list(ctx),
            CliArtifactType::CopyPasteText => Ok(self.convert_copy_paste_text(ctx)),
            CliArtifactType::Text
            | CliArtifactType::PresentationCard
            | CliArtifactType::Dashboard
            | CliArtifactType::Chart
            | CliArtifactType::Form => Ok(self.convert_text(ctx)),
        }
    }

    fn convert_table(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError> {
        let column_names = self
            .hints
            .as_ref()
            .and_then(|h| h.columns.as_ref())
            .ok_or(ConversionError::MissingColumns)?;

        let items = extract_array_from_value(&self.data)?;

        let columns: Vec<Column> = column_names
            .iter()
            .map(|name| Column::new(name, ColumnType::String))
            .collect();

        let artifact = TableArtifact::new(columns, ctx).with_rows(items);

        Ok(CliArtifact::Table { artifact })
    }

    fn convert_list(&self, ctx: &RequestContext) -> Result<CliArtifact, ConversionError> {
        let items = extract_array_from_value(&self.data)?;

        let list_items: Vec<ListItem> = items
            .iter()
            .filter_map(|item| {
                let title = item
                    .get("title")
                    .or_else(|| item.get("name"))
                    .and_then(|v| v.as_str())?;

                let summary = item
                    .get("summary")
                    .or_else(|| item.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let link = item
                    .get("link")
                    .or_else(|| item.get("url"))
                    .or_else(|| item.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Some(ListItem::new(title, summary, link))
            })
            .collect();

        let artifact = ListArtifact::new(ctx).with_items(list_items);

        Ok(CliArtifact::List { artifact })
    }

    fn convert_text(&self, ctx: &RequestContext) -> CliArtifact {
        let content = self
            .data
            .get("message")
            .and_then(|v| v.as_str())
            .map_or_else(
                || {
                    serde_json::to_string_pretty(&self.data)
                        .unwrap_or_else(|_| self.data.to_string())
                },
                String::from,
            );

        let mut artifact = TextArtifact::new(&content, ctx);

        if let Some(title) = &self.title {
            artifact = artifact.with_title(title);
        }

        CliArtifact::Text { artifact }
    }

    fn convert_copy_paste_text(&self, ctx: &RequestContext) -> CliArtifact {
        let content = self
            .data
            .get("content")
            .or_else(|| self.data.get("message"))
            .and_then(|v| v.as_str())
            .map_or_else(
                || {
                    serde_json::to_string_pretty(&self.data)
                        .unwrap_or_else(|_| self.data.to_string())
                },
                String::from,
            );

        let mut artifact = CopyPasteTextArtifact::new(&content, ctx);

        if let Some(title) = &self.title {
            artifact = artifact.with_title(title);
        }

        CliArtifact::CopyPasteText { artifact }
    }
}

fn extract_array_from_value(value: &JsonValue) -> Result<Vec<JsonValue>, ConversionError> {
    if let Some(arr) = value.as_array() {
        return Ok(arr.clone());
    }

    if let Some(obj) = value.as_object() {
        for v in obj.values() {
            if let Some(arr) = v.as_array() {
                return Ok(arr.clone());
            }
        }
    }

    Err(ConversionError::NoArrayFound)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "artifact_type", rename_all = "snake_case")]
pub enum CliArtifact {
    Table {
        #[serde(flatten)]
        artifact: TableArtifact,
    },
    List {
        #[serde(flatten)]
        artifact: ListArtifact,
    },
    Text {
        #[serde(flatten)]
        artifact: TextArtifact,
    },
    #[serde(rename = "copy_paste_text")]
    CopyPasteText {
        #[serde(flatten)]
        artifact: CopyPasteTextArtifact,
    },
    Dashboard {
        #[serde(flatten)]
        artifact: DashboardArtifact,
    },
}

impl CliArtifact {
    #[must_use]
    pub const fn artifact_type_str(&self) -> &'static str {
        match self {
            Self::Table { .. } => "table",
            Self::List { .. } => "list",
            Self::Text { .. } => "text",
            Self::CopyPasteText { .. } => "copy_paste_text",
            Self::Dashboard { .. } => "dashboard",
        }
    }

    #[must_use]
    pub const fn table(artifact: TableArtifact) -> Self {
        Self::Table { artifact }
    }

    #[must_use]
    pub const fn list(artifact: ListArtifact) -> Self {
        Self::List { artifact }
    }

    #[must_use]
    pub const fn text(artifact: TextArtifact) -> Self {
        Self::Text { artifact }
    }

    #[must_use]
    pub const fn copy_paste_text(artifact: CopyPasteTextArtifact) -> Self {
        Self::CopyPasteText { artifact }
    }

    #[must_use]
    pub const fn dashboard(artifact: DashboardArtifact) -> Self {
        Self::Dashboard { artifact }
    }
}
