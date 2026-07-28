//! Dashboard artifact renderer.
//!
//! [`DashboardRenderer`] composes a typed [`DashboardArtifact`] payload into a
//! single HTML [`UiResource`], supporting vertical, grid, and tabbed layouts
//! and embedding Chart.js configurations for any chart sections. Individual
//! section rendering lives in the `section` submodule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod section;

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
use systemprompt_models::artifacts::dashboard::{
    ChartSectionData, DashboardArtifact, DashboardSection, LayoutMode, SectionType,
};

const CHART_JS_CDN: &str = "https://cdn.jsdelivr.net/npm/chart.js@4.4.1/dist/chart.umd.min.js";

#[derive(Debug, Clone, Copy, Default)]
pub struct DashboardRenderer;

impl DashboardRenderer {
    pub const fn new() -> Self {
        Self
    }

    fn build_tabs_nav(sections: &[&DashboardSection]) -> String {
        let tabs = sections
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (i, s)| {
                let active = if i == 0 { " active" } else { "" };
                acc.push_str(&format!(
                    r#"<button class="tab-btn{active}" data-target="{id}">{title}</button>"#,
                    active = active,
                    id = html_escape(s.section_id.as_str()),
                    title = html_escape(&s.title),
                ));
                acc
            });

        format!(r#"<div class="tabs-nav">{tabs}</div>"#)
    }

    fn build_chart_configs(sections: &[&DashboardSection]) -> McpDomainResult<Vec<JsonValue>> {
        sections
            .iter()
            .filter(|s| matches!(s.section_type, SectionType::Chart))
            .map(|s| {
                let chart: ChartSectionData =
                    serde_json::from_value(s.data.clone()).map_err(|e| {
                        crate::error::McpDomainError::Internal(format!(
                            "Dashboard section '{}' data does not match its declared type: {e}",
                            s.section_id.as_str()
                        ))
                    })?;

                Ok(serde_json::json!({
                    "id": format!("chart-{}", s.section_id.as_str()),
                    "type": chart.chart_type,
                    "data": {
                        "labels": chart.labels,
                        "datasets": chart.datasets
                    },
                    "options": {
                        "responsive": true,
                        "maintainAspectRatio": false
                    }
                }))
            })
            .collect()
    }
}

#[async_trait]
impl UiRenderer for DashboardRenderer {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::Dashboard
    }

    async fn render(&self, artifact: &Artifact) -> McpDomainResult<UiResource> {
        let dashboard: DashboardArtifact = typed::artifact_payload(artifact)?;

        let mut sections: Vec<&DashboardSection> = dashboard.sections.iter().collect();
        sections.sort_by_key(|s| s.layout.order);

        let layout_class = match dashboard.hints.layout {
            LayoutMode::Vertical => "layout-vertical",
            LayoutMode::Grid => "layout-grid",
            LayoutMode::Tabs => "layout-tabs",
        };

        let sections_html = sections
            .iter()
            .map(|s| section::render_section(s))
            .collect::<McpDomainResult<Vec<_>>>()?
            .join("\n");

        let tabs_nav = if matches!(dashboard.hints.layout, LayoutMode::Tabs) {
            Self::build_tabs_nav(&sections)
        } else {
            String::new()
        };

        let chart_configs = Self::build_chart_configs(&sections)?;

        let body = format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    {tabs_nav}
    <div class="dashboard {layout_class}">
        {sections}
    </div>
</div>"#,
            title_html = if dashboard.title.is_empty() {
                String::new()
            } else {
                format!(
                    r#"<h1 class="mcp-app-title">{}</h1>"#,
                    html_escape(&dashboard.title)
                )
            },
            description_html =
                dashboard
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

        let html = HtmlBuilder::new(&dashboard.title)
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
