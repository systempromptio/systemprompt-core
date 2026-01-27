use super::html::{base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script, HtmlBuilder};
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

#[derive(Debug, Clone, Copy, Default)]
pub struct TableRenderer;

impl TableRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_table_data(artifact: &Artifact) -> (Vec<String>, Vec<Vec<JsonValue>>) {
        let mut columns = Vec::new();
        let mut rows = Vec::new();

        for part in &artifact.parts {
            if let Some(data) = part.as_data() {
                if let Some(arr) = data.as_array() {
                    for (i, item) in arr.iter().enumerate() {
                        if let Some(obj) = item.as_object() {
                            if i == 0 {
                                columns = obj.keys().cloned().collect();
                            }
                            let row: Vec<JsonValue> =
                                columns.iter().map(|k| obj.get(k).cloned().unwrap_or(JsonValue::Null)).collect();
                            rows.push(row);
                        }
                    }
                } else if let Some(obj) = data.as_object() {
                    if let Some(data_arr) = obj.get("data").or_else(|| obj.get("rows")).and_then(JsonValue::as_array) {
                        if let Some(cols) = obj.get("columns").and_then(JsonValue::as_array) {
                            columns = cols
                                .iter()
                                .filter_map(|c| c.as_str().map(String::from).or_else(|| c.get("name").and_then(|n| n.as_str()).map(String::from)))
                                .collect();
                        }

                        for item in data_arr {
                            if let Some(row_obj) = item.as_object() {
                                let row: Vec<JsonValue> =
                                    columns.iter().map(|k| row_obj.get(k).cloned().unwrap_or(JsonValue::Null)).collect();
                                rows.push(row);
                            } else if let Some(row_arr) = item.as_array() {
                                rows.push(row_arr.clone());
                            }
                        }
                    }
                }
            }
        }

        if columns.is_empty() && !rows.is_empty() {
            columns = (0..rows[0].len()).map(|i| format!("Column {}", i + 1)).collect();
        }

        (columns, rows)
    }

    fn extract_hints(artifact: &Artifact) -> TableHints {
        let mut hints = TableHints::default();

        if let Some(rendering_hints) = &artifact.metadata.rendering_hints {
            if let Some(sortable) = rendering_hints.get("sortable_columns").and_then(JsonValue::as_array) {
                hints.sortable_columns = sortable.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            }
            if let Some(filterable) = rendering_hints.get("filterable").and_then(JsonValue::as_bool) {
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

#[async_trait]
impl UiRenderer for TableRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Table
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let (columns, rows) = Self::extract_table_data(artifact);
        let hints = Self::extract_hints(artifact);
        let title = artifact.name.as_deref().unwrap_or("Table");

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
                .map(|d| format!(r#"<p class="mcp-app-description">{}</p>"#, html_escape(d)))
                .unwrap_or_default(),
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
            "{bridge}\nwindow.TABLE_COLUMNS = {columns};\nwindow.TABLE_ROWS = {rows};\nwindow.TABLE_SORTABLE = {sortable};\nwindow.TABLE_FILTERABLE = {filterable};\nwindow.TABLE_PAGE_SIZE = {page_size};\n{app}",
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
