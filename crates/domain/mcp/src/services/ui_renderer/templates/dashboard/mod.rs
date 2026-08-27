//! Dashboard artifact renderer.
//!
//! [`DashboardRenderer`] composes a typed [`DashboardArtifact`] payload into a
//! single HTML [`UiResource`], supporting vertical, grid, and tabbed layouts.
//! Chart sections are inline SVG like every other chart, so a dashboard needs
//! no script beyond tab switching. Individual section rendering lives in the
//! `section` submodule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod section;

use super::html::{HtmlBuilder, base_styles, html_escape, mcp_app_bridge_script};
use super::typed;
use crate::error::McpDomainResult;
use crate::services::ui_renderer::{CspPolicy, UiRenderer, UiResource};
use async_trait::async_trait;
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::ArtifactType;
use systemprompt_models::artifacts::dashboard::{DashboardArtifact, DashboardSection, LayoutMode};

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
                let selected = i == 0;
                // Why: Roving tabindex: only the selected tab is in the tab order,
                // and the arrow keys move between them. Without the ARIA below
                // these were unlabelled buttons whose selected state lived in a
                // CSS class no assistive tech could see.
                acc.push_str(&format!(
                    r#"<button type="button" class="tab-btn{active}" role="tab" id="tab-{id}" data-target="{id}" aria-controls="{id}" aria-selected="{selected}" tabindex="{tabindex}">{title}</button>"#,
                    active = if selected { " active" } else { "" },
                    id = html_escape(s.section_id.as_str()),
                    selected = selected,
                    tabindex = if selected { "0" } else { "-1" },
                    title = html_escape(&s.title),
                ));
                acc
            });

        format!(r#"<div class="tabs-nav" role="tablist">{tabs}</div>"#)
    }

    fn build_body(
        dashboard: &DashboardArtifact,
        sections_html: &str,
        tabs_nav: &str,
        layout_class: &str,
    ) -> String {
        format!(
            r#"<div class="container">
    {title_html}
    {description_html}
    {refresh_html}
    {tabs_nav}
    <div class="dashboard {layout_class}"{drill}>
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
            // Why: Three of the four DashboardHints fields were parsed and dropped.
            refresh_html = if dashboard.hints.refreshable {
                format!(
                    r#"<div class="dashboard-toolbar"><button type="button" class="refresh-btn" id="dashboard-refresh"{interval}>Refresh</button><span class="refresh-status" id="refresh-status" role="status" aria-live="polite"></span></div>"#,
                    interval = dashboard
                        .hints
                        .refresh_interval_seconds
                        .map_or_else(String::new, |secs| format!(
                            r#" data-refresh-interval="{secs}""#
                        )),
                )
            } else {
                String::new()
            },
            drill = if dashboard.hints.drill_down_enabled {
                r#" data-drill-down="true""#
            } else {
                ""
            },
            sections = sections_html,
        )
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

        // Why: One malformed section used to abort the whole dashboard, so a single
        // bad payload blanked every good section beside it. Each section now
        // fails on its own and says so in place.
        let sections_html = sections
            .iter()
            .map(|s| {
                section::render_section(s).unwrap_or_else(|e| {
                    tracing::warn!(
                        section = %s.section_id,
                        error = %e,
                        "dashboard section failed to render; showing an error card in its place"
                    );
                    section::render_section_error(s, &e.to_string())
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        let tabs_nav = if matches!(dashboard.hints.layout, LayoutMode::Tabs) {
            Self::build_tabs_nav(&sections)
        } else {
            String::new()
        };

        let body = Self::build_body(&dashboard, &sections_html, &tabs_nav, layout_class);

        let script = format!(
            "{bridge}\n{app}",
            bridge = mcp_app_bridge_script(),
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
        CspPolicy::strict()
    }
}

const fn dashboard_styles() -> &'static str {
    include_str!("../assets/css/dashboard.css")
}
