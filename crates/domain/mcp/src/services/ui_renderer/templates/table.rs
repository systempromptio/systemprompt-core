//! Table artifact renderer.
//!
//! [`TableRenderer`] renders a tabular [`Artifact`] into an HTML
//! [`UiResource`], inferring columns and rows from row-object arrays or
//! explicit column/row definitions and wiring client-side sorting,
//! filtering, and pagination from the artifact's rendering hints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{
    HtmlBuilder, base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script,
};
use super::typed::{lenient, lenient_vec};
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, serde::Serialize)]
struct TableColumn {
    key: String,
    header: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    align: Option<String>,
}

impl TableColumn {
    fn from_key(key: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            header: humanize(&key),
            key,
            kind: "string".to_owned(),
            align: None,
        }
    }

    fn from_spec(spec: ColumnSpec) -> Option<Self> {
        let spec = match spec {
            ColumnSpec::Key(name) => return Some(Self::from_key(name)),
            ColumnSpec::Fields(spec) => spec,
        };

        let key = spec.name?;
        Some(Self {
            header: spec.label.or(spec.header).unwrap_or_else(|| humanize(&key)),
            key,
            kind: spec
                .column_type
                .or(spec.kind)
                .unwrap_or_else(|| "string".to_owned()),
            align: spec.align,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ColumnSpec {
    Key(String),
    Fields(Box<ColumnFields>),
}

#[derive(Debug, Deserialize)]
struct ColumnFields {
    #[serde(default, deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    label: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    header: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    column_type: Option<String>,
    #[serde(default, rename = "type", deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    align: Option<String>,
}

fn humanize(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for (i, word) in key.split(['_', '-']).filter(|w| !w.is_empty()).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.extend(first.to_uppercase());
            } else {
                out.push(first);
            }
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() { key.to_owned() } else { out }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TableRenderer;

impl TableRenderer {
    pub const fn new() -> Self {
        Self
    }

    // JSON: table artifact cells — producer rows hold values of any JSON type.
    fn extract_table_data(artifact: &Artifact) -> (Vec<TableColumn>, Vec<Vec<JsonValue>>) {
        let mut columns: Vec<TableColumn> = Vec::new();
        let mut rows = Vec::new();

        for part in &artifact.parts {
            if let Some(data) = part.as_data()
                && let Some(obj) = data.as_object()
                && let Some(data_arr) = obj
                    .get("items")
                    .or_else(|| obj.get("data"))
                    .or_else(|| obj.get("rows"))
                    .and_then(JsonValue::as_array)
            {
                if let Some(cols) = obj.get("columns").filter(|c| c.is_array()) {
                    columns = lenient_vec::<_, ColumnSpec>(cols)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(TableColumn::from_spec)
                        .collect();
                }

                if columns.is_empty()
                    && let Some(first_obj) = data_arr.first().and_then(JsonValue::as_object)
                {
                    columns = first_obj.keys().map(TableColumn::from_key).collect();
                }

                for item in data_arr {
                    if let Some(row_obj) = item.as_object() {
                        let row: Vec<JsonValue> = columns
                            .iter()
                            .map(|c| row_obj.get(&c.key).cloned().unwrap_or(JsonValue::Null))
                            .collect();
                        rows.push(row);
                    } else if let Some(row_arr) = item.as_array() {
                        rows.push(row_arr.clone());
                    }
                }
            }
        }

        if columns.is_empty() && !rows.is_empty() {
            columns = (0..rows[0].len())
                .map(|i| TableColumn::from_key(format!("column_{}", i + 1)))
                .collect();
        }

        (columns, rows)
    }

    fn extract_hints(artifact: &Artifact) -> TableHints {
        let mut hints = TableHints::default();

        if let Some(rendering_hints) = &artifact.metadata.rendering_hints {
            if let Some(sortable) = rendering_hints
                .get("sortable_columns")
                .and_then(JsonValue::as_array)
            {
                hints.sortable_columns = sortable
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }
            if let Some(filterable) = rendering_hints
                .get("filterable")
                .and_then(JsonValue::as_bool)
            {
                hints.filterable = filterable;
            }
            if let Some(page_size) = rendering_hints.get("page_size").and_then(JsonValue::as_u64) {
                hints.page_size = page_size as usize;
            }
        }

        hints
    }
}

#[derive(Default)]
struct TableHints {
    sortable_columns: Vec<String>,
    filterable: bool,
    page_size: usize,
}

impl UiRenderer for TableRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Table
    }

    fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let (columns, rows) = Self::extract_table_data(artifact);
        let hints = Self::extract_hints(artifact);
        let title = artifact.title.as_deref().unwrap_or("Table");

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    {filter_html}
    <div class="table-wrapper">
        <table class="data-table" id="data-table">
            <thead id="table-head"></thead>
            <tbody id="table-body"></tbody>
        </table>
    </div>
    {pagination_html}
</div>"#,
            title_html = if title.is_empty() {
                String::new()
            } else {
                format!(r#"<h1 class="mcp-app-title">{}</h1>"#, html_escape(title))
            },
            description_html = artifact
                .description
                .as_ref()
                .map_or_else(String::new, |d| format!(
                    r#"<p class="mcp-app-description">{}</p>"#,
                    html_escape(d)
                )),
            filter_html = if hints.filterable {
                r#"<div class="filter-bar">
                    <input type="text" id="filter-input" placeholder="Filter..." class="filter-input">
                </div>"#
            } else {
                ""
            },
            pagination_html = if hints.page_size > 0 {
                r#"<div class="pagination" id="pagination"></div>"#
            } else {
                ""
            }
        );

        let script = format!(
            "{bridge}\nwindow.TABLE_COLUMNS = {columns};\nwindow.TABLE_ROWS = \
             {rows};\nwindow.TABLE_SORTABLE = {sortable};\nwindow.TABLE_FILTERABLE = \
             {filterable};\nwindow.TABLE_PAGE_SIZE = {page_size};\n{app}",
            bridge = mcp_app_bridge_script(),
            columns = json_to_js_literal(&serde_json::json!(columns)),
            rows = json_to_js_literal(&serde_json::json!(rows)),
            sortable = json_to_js_literal(&serde_json::json!(hints.sortable_columns)),
            filterable = hints.filterable,
            page_size = hints.page_size,
            app = include_str!("assets/js/table.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(table_styles())
            .body(&body)
            .add_script(&script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

const fn table_styles() -> &'static str {
    include_str!("assets/css/table.css")
}
