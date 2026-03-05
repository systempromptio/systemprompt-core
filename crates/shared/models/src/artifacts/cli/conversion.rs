use serde_json::Value as JsonValue;

use super::{CliArtifact, CliArtifactType, CommandResultRaw, ConversionError};
use crate::artifacts::list::ListItem;
use crate::artifacts::table::Column;
use crate::artifacts::types::ColumnType;
use crate::artifacts::{
    CopyPasteTextArtifact, ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
};
use crate::execution::context::RequestContext;

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
            CliArtifactType::PresentationCard => self.convert_presentation_card(ctx),
            CliArtifactType::Text
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

    fn convert_presentation_card(
        &self,
        _ctx: &RequestContext,
    ) -> Result<CliArtifact, ConversionError> {
        let artifact: PresentationCardArtifact =
            serde_json::from_value(self.data.clone()).map_err(ConversionError::Json)?;
        Ok(CliArtifact::PresentationCard { artifact })
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
