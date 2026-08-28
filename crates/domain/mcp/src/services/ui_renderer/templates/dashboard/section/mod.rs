//! Per-section HTML rendering for the dashboard renderer.
//!
//! Renders one typed [`DashboardSection`] according to its [`SectionType`],
//! deserializing the section's `data` into the matching payload struct and
//! producing the inner markup the dashboard renderer assembles into the full
//! page. A section whose `data` does not match its declared type is an error,
//! not an empty body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blocks;
mod tables;

use super::super::html::html_escape;
use crate::error::{McpDomainError, McpDomainResult};
use blocks::{
    render_chart, render_list, render_metrics, render_status, render_text, render_timeline,
};
use serde::de::DeserializeOwned;
use systemprompt_models::artifacts::dashboard::{DashboardSection, LayoutWidth, SectionType};
use tables::render_table;


pub(super) fn render_section(section: &DashboardSection) -> McpDomainResult<String> {
    let content = match section.section_type {
        SectionType::MetricsCards => render_metrics(section)?,
        SectionType::Chart => render_chart(section)?,
        SectionType::Table => render_table(section)?,
        SectionType::Status => render_status(section)?,
        SectionType::List => render_list(section)?,
        SectionType::Timeline => render_timeline(section)?,
        SectionType::Text => render_text(section)?,
    };

    Ok(format!(
        r#"<div class="dashboard-section" id="{id}" style="--section-span: {width}">
    <h2 class="section-title">{title}</h2>
    <div class="section-content">
        {content}
    </div>
</div>"#,
        id = html_escape(section.section_id.as_str()),
        width = width_span(section.layout.width),
        title = html_escape(&section.title),
        content = content,
    ))
}

const fn width_span(width: LayoutWidth) -> &'static str {
    match width {
        LayoutWidth::Full => "12",
        LayoutWidth::Half => "6",
        LayoutWidth::Third => "4",
        LayoutWidth::TwoThirds => "8",
    }
}

pub(super) fn status_class(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "ok" | "healthy" | "success" | "active" | "running" => "status-ok",
        "warning" | "degraded" => "status-warning",
        "error" | "failed" | "critical" => "status-error",
        _ => "status-unknown",
    }
}

pub(super) fn section_data<T: DeserializeOwned>(section: &DashboardSection) -> McpDomainResult<T> {
    serde_json::from_value(section.data.clone()).map_err(|e| {
        McpDomainError::Internal(format!(
            "Dashboard section '{}' data does not match its declared type: {e}",
            section.section_id.as_str()
        ))
    })
}

pub(super) fn render_section_error(section: &DashboardSection, detail: &str) -> String {
    format!(
        r#"<div class="dashboard-section" id="{id}" style="--section-span: {width}">
    <h2 class="section-title">{title}</h2>
    <div class="section-content">
        <p class="error-message">This section could not be displayed. {detail}</p>
    </div>
</div>"#,
        id = html_escape(section.section_id.as_str()),
        width = width_span(section.layout.width),
        title = html_escape(&section.title),
        detail = html_escape(detail),
    )
}

pub(super) fn section_empty(message: &str) -> String {
    format!(r#"<p class="section-empty">{}</p>"#, html_escape(message))
}
