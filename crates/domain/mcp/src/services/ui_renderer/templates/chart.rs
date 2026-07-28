//! Chart artifact renderer.
//!
//! [`ChartRenderer`] turns a typed [`ChartArtifact`] payload into a
//! self-contained HTML [`UiResource`] backed by Chart.js, mapping the chart
//! type, title, axes, and datasets into a Chart.js configuration and emitting
//! a CSP that permits the Chart.js CDN.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::html::{
    HtmlBuilder, base_styles, html_escape, json_to_js_literal, mcp_app_bridge_script,
};
use super::typed;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspBuilder, CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;
use systemprompt_models::artifacts::chart::ChartArtifact;
use systemprompt_models::artifacts::types::ChartType;

const CHART_JS_CDN: &str = "https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js";

#[derive(Debug, Clone, Copy, Default)]
pub struct ChartRenderer;

impl ChartRenderer {
    pub const fn new() -> Self {
        Self
    }
}

fn chartjs_config(chart: &ChartArtifact) -> JsonValue {
    let chart_type = match chart.chart_type {
        ChartType::Line | ChartType::Area => "line",
        ChartType::Pie => "pie",
        ChartType::Doughnut => "doughnut",
        ChartType::Bar => "bar",
    };

    let is_area = matches!(chart.chart_type, ChartType::Area);

    let datasets: Vec<JsonValue> = chart
        .datasets
        .iter()
        .map(|ds| {
            let mut dataset = serde_json::json!(ds);
            if is_area && let Some(obj) = dataset.as_object_mut() {
                obj.insert("fill".to_owned(), JsonValue::Bool(true));
            }
            dataset
        })
        .collect();

    let mut config = serde_json::json!({
        "type": chart_type,
        "data": {
            "labels": chart.labels,
            "datasets": datasets
        },
        "options": {
            "responsive": true,
            "maintainAspectRatio": false,
            "plugins": {
                "legend": {
                    "position": "top"
                }
            }
        }
    });

    if !chart.title.is_empty() {
        config["options"]["plugins"]["title"] = serde_json::json!({
            "display": true,
            "text": chart.title
        });
    }

    let mut scales = serde_json::json!({});
    if !chart.x_axis_label.is_empty() {
        scales["x"] = serde_json::json!({
            "title": { "display": true, "text": chart.x_axis_label }
        });
    }
    if !chart.y_axis_label.is_empty() {
        scales["y"] = serde_json::json!({
            "title": { "display": true, "text": chart.y_axis_label }
        });
    }
    if scales.as_object().is_some_and(|s| !s.is_empty()) {
        config["options"]["scales"] = scales;
    }

    config
}

#[async_trait]
impl UiRenderer for ChartRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Chart
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let chart: ChartArtifact = typed::artifact_payload(artifact)?;
        let title = if chart.title.is_empty() {
            artifact.title.as_deref().unwrap_or("Chart")
        } else {
            chart.title.as_str()
        };
        let chartjs_config = chartjs_config(&chart);

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    <div class="chart-wrapper">
        <canvas id="chart"></canvas>
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
        );

        let script = format!(
            "{bridge}\nwindow.CHART_CONFIG = {config};\nwindow.CHART_JS_CDN = '{cdn}';\n{app}",
            bridge = mcp_app_bridge_script(),
            config = json_to_js_literal(&chartjs_config),
            cdn = CHART_JS_CDN,
            app = include_str!("assets/js/chart.js"),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(chart_styles())
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

const fn chart_styles() -> &'static str {
    include_str!("assets/css/chart.css")
}
