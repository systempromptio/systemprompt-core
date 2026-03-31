mod section;

use super::html::{
    HtmlBuilder, base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script,
};
use crate::services::ui_renderer::{CspBuilder, CspPolicy, UiRenderer, UiResource};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;

use section::{DashboardSection, SectionType};

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

#[derive(Debug, Clone, Copy)]
enum DashboardLayout {
    Vertical,
    Grid,
    Tabs,
}

pub(super) fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u32)
}

#[async_trait]
impl UiRenderer for DashboardRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Dashboard
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        use std::fmt::Write;

        let sections = Self::extract_sections(artifact);
        let layout = Self::extract_layout(artifact);
        let title = artifact.name.as_deref().unwrap_or("Dashboard");

        let layout_class = match layout {
            DashboardLayout::Vertical => "layout-vertical",
            DashboardLayout::Grid => "layout-grid",
            DashboardLayout::Tabs => "layout-tabs",
        };

        let sections_html: String = sections.iter().map(DashboardSection::render_html).collect();

        let tabs_nav = if matches!(layout, DashboardLayout::Tabs) {
            let tabs = sections
                .iter()
                .enumerate()
                .fold(String::new(), |mut acc, (i, s)| {
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

        let chart_configs: Vec<JsonValue> = sections
            .iter()
            .filter(|s| matches!(s.section_type, SectionType::Chart))
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
                .map_or_else(String::new, |d| format!(
                    r#"<p class="mcp-app-description">{}</p>"#,
                    html_escape(d)
                )),
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
            app = include_str!("../assets/js/dashboard.js"),
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
    include_str!("../assets/css/dashboard.css")
}
