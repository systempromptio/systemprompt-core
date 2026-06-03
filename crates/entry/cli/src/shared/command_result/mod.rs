//! Structured command output and its terminal/JSON rendering.
//!
//! [`CommandOutput`] wraps the typed [`CliArtifact`] a command produces.
//! Machine output (`--output json`/`yaml`) serializes the artifact verbatim —
//! the same tagged union the MCP server deserializes. [`render_result`] renders
//! the artifact for an interactive terminal, dispatching per variant. The
//! reusable payload shapes [`TextOutput`], [`SuccessOutput`],
//! [`KeyValueOutput`], and [`TableOutput`] remain available as command-side
//! data structs.

mod output_types;
mod render;

pub use output_types::{KeyValueItem, KeyValueOutput, SuccessOutput, TableOutput, TextOutput};
pub use render::render_result;
pub use systemprompt_models::artifacts::ChartType;

use serde::Serialize;
use serde_json::Value as JsonValue;
use systemprompt_models::artifacts::{
    CardSection, ChartArtifact, CliArtifact, Column, ColumnType, CopyPasteTextArtifact,
    DashboardArtifact, ListArtifact, ListItem, MessageArtifact, NoticeLine,
    PresentationCardArtifact, TableArtifact, TextArtifact,
};

/// A command's renderable result: a typed [`CliArtifact`] plus terminal-only
/// presentation state (an optional section title and a render-suppression
/// flag).
///
/// The artifact is the single source of truth on the wire; `title` and
/// `skip_render` only affect interactive terminal rendering.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    artifact: CliArtifact,
    title: Option<String>,
    skip_render: bool,
}

impl CommandOutput {
    #[must_use]
    pub const fn new(artifact: CliArtifact) -> Self {
        Self {
            artifact,
            title: None,
            skip_render: false,
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> &CliArtifact {
        &self.artifact
    }

    #[must_use]
    pub fn into_artifact(self) -> CliArtifact {
        self.artifact
    }

    #[must_use]
    pub const fn should_skip_render(&self) -> bool {
        self.skip_render
    }

    #[must_use]
    pub const fn with_skip_render(mut self) -> Self {
        self.skip_render = true;
        self
    }

    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::new(CliArtifact::text(TextArtifact::new(content)))
    }

    #[must_use]
    pub fn text_titled(title: impl Into<String>, content: impl Into<String>) -> Self {
        let title = title.into();
        Self::new(CliArtifact::text(
            TextArtifact::new(content).with_title(title.clone()),
        ))
        .with_title(title)
    }

    #[must_use]
    pub fn copy_paste(content: impl Into<String>) -> Self {
        Self::new(CliArtifact::copy_paste_text(CopyPasteTextArtifact::new(
            content,
        )))
    }

    #[must_use]
    pub fn copy_paste_titled(title: impl Into<String>, content: impl Into<String>) -> Self {
        let title = title.into();
        Self::new(CliArtifact::copy_paste_text(
            CopyPasteTextArtifact::new(content).with_title(title.clone()),
        ))
        .with_title(title)
    }

    #[must_use]
    pub fn table(columns: Vec<impl Into<String>>, rows: Vec<JsonValue>) -> Self {
        let cols: Vec<Column> = columns
            .into_iter()
            .map(|c| Column::new(c, ColumnType::String))
            .collect();
        Self::new(CliArtifact::table(TableArtifact::new(cols).with_rows(rows)))
    }

    /// Build a table by serializing each row item to a JSON object. `columns`
    /// names the fields to display; the renderer reads them off each object.
    #[must_use]
    pub fn table_of<T: Serialize>(columns: Vec<impl Into<String>>, items: &[T]) -> Self {
        let rows: Vec<JsonValue> = items
            .iter()
            .map(|item| serde_json::to_value(item).unwrap_or(JsonValue::Null))
            .collect();
        Self::table(columns, rows)
    }

    #[must_use]
    pub const fn table_artifact(artifact: TableArtifact) -> Self {
        Self::new(CliArtifact::table(artifact))
    }

    #[must_use]
    pub fn list(items: Vec<ListItem>) -> Self {
        Self::new(CliArtifact::list(ListArtifact::new().with_items(items)))
    }

    #[must_use]
    pub const fn card(card: PresentationCardArtifact) -> Self {
        Self::new(CliArtifact::presentation_card(card))
    }

    /// Build a presentation card whose sections are the top-level fields of a
    /// serializable value (one `CardSection` per field). Deterministic
    /// producer-side mapping — the wire carries a concrete card.
    #[must_use]
    pub fn card_value(title: impl Into<String>, value: &impl Serialize) -> Self {
        let sections = sections_from_value(&serde_json::to_value(value).unwrap_or(JsonValue::Null));
        Self::card(PresentationCardArtifact::new(title).with_sections(sections))
    }

    #[must_use]
    pub const fn chart(chart: ChartArtifact) -> Self {
        Self::new(CliArtifact::chart(chart))
    }

    #[must_use]
    pub const fn dashboard(dashboard: DashboardArtifact) -> Self {
        Self::new(CliArtifact::dashboard(dashboard))
    }

    #[must_use]
    pub fn message(lines: Vec<NoticeLine>) -> Self {
        Self::new(CliArtifact::message(MessageArtifact::new(lines)))
    }
}

impl From<CliArtifact> for CommandOutput {
    fn from(artifact: CliArtifact) -> Self {
        Self::new(artifact)
    }
}

/// Turn a JSON value into card sections: one section per top-level object
/// field. Scalars render as their display string; nested arrays/objects as
/// compact JSON. A non-object value yields a single `Value` section.
fn sections_from_value(value: &JsonValue) -> Vec<CardSection> {
    match value {
        JsonValue::Object(map) => map
            .iter()
            .map(|(key, val)| CardSection::new(key.clone(), value_to_display(val)))
            .collect(),
        JsonValue::Null => Vec::new(),
        other => vec![CardSection::new("Value", value_to_display(other))],
    }
}

fn value_to_display(value: &JsonValue) -> String {
    match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Null => String::new(),
        JsonValue::Bool(_) | JsonValue::Number(_) => value.to_string(),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
        },
    }
}
