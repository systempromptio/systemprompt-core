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
pub struct ChartRenderer;

impl ChartRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn extract_chart_config(artifact: &Artifact) -> ChartConfig {
        let mut config = ChartConfig::default();

        if let Some(hints) = &artifact.metadata.rendering_hints {
            if let Some(chart_type) = hints.get("chart_type").and_then(JsonValue::as_str) {
                config.chart_type = chart_type.to_string();
            }
            if let Some(title) = hints.get("title").and_then(JsonValue::as_str) {
                config.title = Some(title.to_string());
            }
            if let Some(x_label) = hints.get("x_axis_label").and_then(JsonValue::as_str) {
                config.x_axis_label = Some(x_label.to_string());
            }
            if let Some(y_label) = hints.get("y_axis_label").and_then(JsonValue::as_str) {
                config.y_axis_label = Some(y_label.to_string());
            }
        }

        for part in &artifact.parts {
            if let Some(data) = part.as_data() {
                if let Some(labels) = data.get("labels").and_then(JsonValue::as_array) {
                    config.labels = labels
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }
                if let Some(datasets) = data.get("datasets").and_then(JsonValue::as_array) {
                    config.datasets.clone_from(datasets);
                }
                if let Some(data_arr) = data.get("data").and_then(JsonValue::as_array) {
                    if config.datasets.is_empty() {
                        config.datasets = vec![serde_json::json!({
                            "label": artifact.name.as_deref().unwrap_or("Data"),
                            "data": data_arr
                        })];
                    }
                }
            }
        }

        config
    }
}

#[derive(Default)]
struct ChartConfig {
    chart_type: String,
    title: Option<String>,
    x_axis_label: Option<String>,
    y_axis_label: Option<String>,
    labels: Vec<String>,
    datasets: Vec<JsonValue>,
}

impl ChartConfig {
    fn to_chartjs_config(&self) -> JsonValue {
        let chart_type = match self.chart_type.as_str() {
            "line" | "Line" | "area" | "Area" => "line",
            "pie" | "Pie" => "pie",
            "doughnut" | "Doughnut" => "doughnut",
            _ => "bar",
        };

        let is_area = self.chart_type.to_lowercase() == "area";

        let datasets: Vec<JsonValue> = self
            .datasets
            .iter()
            .map(|ds| {
                let mut dataset = ds.clone();
                if is_area {
                    if let Some(obj) = dataset.as_object_mut() {
                        obj.insert("fill".to_string(), JsonValue::Bool(true));
                    }
                }
                dataset
            })
            .collect();

        let mut config = serde_json::json!({
            "type": chart_type,
            "data": {
                "labels": self.labels,
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

        if let Some(title) = &self.title {
            config["options"]["plugins"]["title"] = serde_json::json!({
                "display": true,
                "text": title
            });
        }

        if self.x_axis_label.is_some() || self.y_axis_label.is_some() {
            let mut scales = serde_json::json!({});
            if let Some(x_label) = &self.x_axis_label {
                scales["x"] = serde_json::json!({
                    "title": { "display": true, "text": x_label }
                });
            }
            if let Some(y_label) = &self.y_axis_label {
                scales["y"] = serde_json::json!({
                    "title": { "display": true, "text": y_label }
                });
            }
            config["options"]["scales"] = scales;
        }

        config
    }
}

#[async_trait]
impl UiRenderer for ChartRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Chart
    }

    async fn render(&self, artifact: &Artifact) -> Result<UiResource> {
        let config = Self::extract_chart_config(artifact);
        let title = artifact.name.as_deref().unwrap_or("Chart");
        let chartjs_config = config.to_chartjs_config();

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
