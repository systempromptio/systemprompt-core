//! Chart artifact renderer.
//!
//! [`ChartRenderer`] turns a typed [`ChartArtifact`] payload into a
//! self-contained HTML [`UiResource`] whose plot is inline SVG produced by
//! [`chart_svg`](super::chart_svg) — no charting library, no network, and a
//! strict CSP.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::chart_svg::{self, ChartSpec};
use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script};
use super::typed;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;
use systemprompt_models::artifacts::chart::ChartArtifact;

#[derive(Debug, Clone, Copy, Default)]
pub struct ChartRenderer;

impl ChartRenderer {
    pub const fn new() -> Self {
        Self
    }
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
        let spec = ChartSpec {
            chart_type: chart.chart_type,
            labels: &chart.labels,
            datasets: &chart.datasets,
            x_axis_label: &chart.x_axis_label,
            y_axis_label: &chart.y_axis_label,
        };

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
{plot}
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
            plot = chart_svg::render(&spec, title),
        );

        let html = HtmlBuilder::new(title)
            .add_style(base_styles())
            .add_style(chart_styles())
            .body(&body)
            .add_script(mcp_app_bridge_script())
            .build();

        Ok(UiResource::new(html).with_csp(self.csp_policy()))
    }

    fn csp_policy(&self) -> CspPolicy {
        CspPolicy::strict()
    }
}

const fn chart_styles() -> &'static str {
    include_str!("assets/css/chart.css")
}
