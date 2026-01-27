use super::html::{
    base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script, HtmlBuilder,
};
use crate::services::ui_renderer::{CspBuilder, CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

const CHART_JS_CDN: &str = "https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js";

#[derive(Debug, Clone, Copy, Default)]
pub struct DashboardRenderer;

impl DashboardRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_sections(artifact: &Artifact) -> Vec<DashboardSection> {
        let mut sections = Vec::new();

        for part in &artifact.parts {
            if let Some(data) = part.as_data() {
                if let Some(sections_arr) = data.get("sections").and_then(JsonValue::as_array) {
                    for section_data in sections_arr {
                        sections.push(DashboardSection::from_json(section_data));
                    }
                } else {
                    sections.push(DashboardSection::from_json(&data));
                }
            }
        }

        sections
    }

    fn extract_layout(artifact: &Artifact) -> DashboardLayout {
        artifact
            .metadata
            .rendering_hints
            .as_ref()
            .and_then(|h| h.get("layout"))
            .and_then(JsonValue::as_str)
            .map_or(DashboardLayout::Vertical, |s| match s {
                "grid" | "Grid" => DashboardLayout::Grid,
                "tabs" | "Tabs" => DashboardLayout::Tabs,
                _ => DashboardLayout::Vertical,
            })
    }
}

#[derive(Debug)]
struct DashboardSection {
    id: String,
    title: String,
    section_type: SectionType,
    data: JsonValue,
    width: Option<String>,
}

#[derive(Debug)]
enum SectionType {
    Metrics,
    Chart,
    Table,
    Status,
    List,
    Text,
}

impl DashboardSection {
    fn from_json(value: &JsonValue) -> Self {
        let title = value
            .get("title")
            .and_then(JsonValue::as_str)
            .unwrap_or("Section")
            .to_string();

        let section_type =
            value
                .get("type")
                .and_then(JsonValue::as_str)
                .map_or(SectionType::Text, |s| match s.to_lowercase().as_str() {
                    "metrics" | "kpi" => SectionType::Metrics,
                    "chart" | "graph" => SectionType::Chart,
                    "table" => SectionType::Table,
                    "status" => SectionType::Status,
                    "list" => SectionType::List,
                    _ => SectionType::Text,
                });

        let id = value
            .get("id")
            .and_then(JsonValue::as_str)
            .map_or_else(|| format!("section-{}", rand_id()), String::from);

        Self {
            id,
            title,
            section_type,
            data: value.clone(),
            width: value
                .get("width")
                .and_then(JsonValue::as_str)
                .map(String::from),
        }
    }

    fn render_html(&self) -> String {
        let content = match &self.section_type {
            SectionType::Metrics => self.render_metrics(),
            SectionType::Chart => self.render_chart(),
            SectionType::Table => self.render_table(),
            SectionType::Status => self.render_status(),
            SectionType::List => self.render_list(),
            SectionType::Text => self.render_text(),
        };

        let width_style = self
            .width
            .as_ref()
            .map(|w| format!(r#" style="flex-basis: {}""#, html_escape(w)))
            .unwrap_or_default();

        format!(
            r#"<div class="dashboard-section" id="{id}"{width}>
    <h2 class="section-title">{title}</h2>
    <div class="section-content">
        {content}
    </div>
</div>"#,
            id = html_escape(&self.id),
            width = width_style,
            title = html_escape(&self.title),
            content = content,
        )
    }

    fn render_metrics(&self) -> String {
        let metrics = self
            .data
            .get("metrics")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let label = m.get("label").or_else(|| m.get("name")).and_then(JsonValue::as_str)?;
                        let value = m.get("value").map(|v| {
                            v.as_f64().map_or_else(
                                || v.to_string().trim_matches('"').to_string(),
                                |n| format!("{:.2}", n),
                            )
                        })?;
                        let change = m.get("change").and_then(JsonValue::as_f64);
                        let unit = m.get("unit").and_then(JsonValue::as_str).unwrap_or("");

                        let change_html = change
                            .map(|c| {
                                let class = if c >= 0.0 { "positive" } else { "negative" };
                                let sign = if c >= 0.0 { "+" } else { "" };
                                format!(r#"<span class="metric-change {class}">{sign}{c:.1}%</span>"#)
                            })
                            .unwrap_or_default();

                        Some(format!(
                            r#"<div class="metric-card">
                                <div class="metric-value">{value}<span class="metric-unit">{unit}</span></div>
                                <div class="metric-label">{label}</div>
                                {change}
                            </div>"#,
                            value = html_escape(&value),
                            unit = html_escape(unit),
                            label = html_escape(label),
                            change = change_html,
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        format!(r#"<div class="metrics-grid">{}</div>"#, metrics)
    }

    fn render_chart(&self) -> String {
        format!(
            r#"<div class="chart-container"><canvas id="chart-{}"></canvas></div>"#,
            html_escape(&self.id)
        )
    }

    fn render_table(&self) -> String {
        let columns = self
            .data
            .get("columns")
            .and_then(JsonValue::as_array)
            .map(|arr| arr.iter().filter_map(|c| c.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();

        let rows = self
            .data
            .get("rows")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array);

        if columns.is_empty() {
            return "<p>No table data</p>".to_string();
        }

        let header = columns.iter().fold(String::new(), |mut acc, c| {
            use std::fmt::Write;
            let _ = write!(acc, "<th>{}</th>", html_escape(c));
            acc
        });

        let body = rows
            .map(|arr| {
                arr.iter()
                    .map(|row| {
                        let cells = row.as_object().map_or_else(
                            || {
                                row.as_array()
                                    .map(|arr| arr.iter().map(ToString::to_string).collect())
                                    .unwrap_or_default()
                            },
                            |obj| {
                                columns
                                    .iter()
                                    .map(|c| {
                                        obj.get(*c).map(ToString::to_string).unwrap_or_default()
                                    })
                                    .collect::<Vec<_>>()
                            },
                        );

                        let cells_html = cells.iter().fold(String::new(), |mut acc, c| {
                            use std::fmt::Write;
                            let _ = write!(acc, "<td>{}</td>", html_escape(c.trim_matches('"')));
                            acc
                        });

                        format!("<tr>{cells_html}</tr>")
                    })
                    .fold(String::new(), |mut acc, row| {
                        acc.push_str(&row);
                        acc
                    })
            })
            .unwrap_or_default();

        format!(
            r#"<table class="section-table">
                <thead><tr>{header}</tr></thead>
                <tbody>{body}</tbody>
            </table>"#,
            header = header,
            body = body,
        )
    }

    fn render_status(&self) -> String {
        let items = self
            .data
            .get("items")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let name = item
                            .get("name")
                            .or_else(|| item.get("label"))
                            .and_then(JsonValue::as_str)?;
                        let status = item
                            .get("status")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("unknown");
                        let status_class = match status.to_lowercase().as_str() {
                            "ok" | "healthy" | "success" | "active" => "status-ok",
                            "warning" | "degraded" => "status-warning",
                            "error" | "failed" | "critical" => "status-error",
                            _ => "status-unknown",
                        };

                        Some(format!(
                            r#"<div class="status-item">
                                <span class="status-indicator {class}"></span>
                                <span class="status-name">{name}</span>
                                <span class="status-value">{status}</span>
                            </div>"#,
                            class = status_class,
                            name = html_escape(name),
                            status = html_escape(status),
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        format!(r#"<div class="status-list">{}</div>"#, items)
    }

    fn render_list(&self) -> String {
        let items = self
            .data
            .get("items")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let text = if let Some(s) = item.as_str() {
                            s.to_string()
                        } else {
                            item.get("text")
                                .or_else(|| item.get("title"))
                                .and_then(JsonValue::as_str)
                                .map(String::from)?
                        };
                        Some(format!("<li>{}</li>", html_escape(&text)))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        format!(r#"<ul class="section-list">{}</ul>"#, items)
    }

    fn render_text(&self) -> String {
        let text = self
            .data
            .get("text")
            .or_else(|| self.data.get("content"))
            .and_then(JsonValue::as_str)
            .unwrap_or("");

        format!(r#"<p class="section-text">{}</p>"#, html_escape(text))
    }
}

#[derive(Debug, Clone, Copy)]
enum DashboardLayout {
    Vertical,
    Grid,
    Tabs,
}

fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(0)
}

#[async_trait]
impl UiRenderer for DashboardRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Dashboard
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let sections = Self::extract_sections(artifact);
        let layout = Self::extract_layout(artifact);
        let title = artifact.name.as_deref().unwrap_or("Dashboard");

        let layout_class = match layout {
            DashboardLayout::Vertical => "layout-vertical",
            DashboardLayout::Grid => "layout-grid",
            DashboardLayout::Tabs => "layout-tabs",
        };

        let sections_html: String = sections.iter().map(DashboardSection::render_html).collect();

        let tabs_nav =
            if matches!(layout, DashboardLayout::Tabs) {
                use std::fmt::Write;
                let tabs = sections.iter().enumerate().fold(String::new(), |mut acc, (i, s)| {
                let active = if i == 0 { " active" } else { "" };
                let _ = write!(
                    acc,
                    r#"<button class="tab-btn{active}" data-target="{id}">{title}</button>"#,
                    active = active,
                    id = html_escape(&s.id),
                    title = html_escape(&s.title),
                );
                acc
            });

                format!(r#"<div class="tabs-nav">{tabs}</div>"#)
            } else {
                String::new()
            };

        let chart_sections: Vec<&DashboardSection> = sections
            .iter()
            .filter(|s| matches!(s.section_type, SectionType::Chart))
            .collect();

        let chart_configs: Vec<JsonValue> = chart_sections
            .iter()
            .map(|s| {
                let chart_type = s
                    .data
                    .get("chart_type")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("bar");
                let labels = s
                    .data
                    .get("labels")
                    .cloned()
                    .unwrap_or(serde_json::json!([]));
                let datasets = s
                    .data
                    .get("datasets")
                    .cloned()
                    .unwrap_or(serde_json::json!([]));

                serde_json::json!({
                    "id": format!("chart-{}", s.id),
                    "type": chart_type,
                    "data": {
                        "labels": labels,
                        "datasets": datasets
                    },
                    "options": {
                        "responsive": true,
                        "maintainAspectRatio": false
                    }
                })
            })
            .collect();

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    {tabs_nav}
    <div class="dashboard {layout_class}">
        {sections}
    </div>
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
            tabs_nav = tabs_nav,
            layout_class = layout_class,
            sections = sections_html,
        );

        let script = format!(
            "{bridge}\nwindow.DASHBOARD_CHART_CONFIGS = {chart_configs};\nwindow.CHART_JS_CDN = \
             '{cdn}';\n{app}",
            bridge = mcp_app_bridge_script(),
            chart_configs = json_to_js_literal(&serde_json::json!(chart_configs)),
            cdn = CHART_JS_CDN,
            app = include_str!("assets/js/dashboard.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(dashboard_styles())
            .body(&body)
            .add_script(&script)
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspBuilder::strict()
            .add_script_src("https://cdn.jsdelivr.net")
            .build()
    }
}

const fn dashboard_styles() -> &'static str {
    include_str!("assets/css/dashboard.css")
}
