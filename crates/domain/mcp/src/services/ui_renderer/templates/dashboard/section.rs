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

use super::super::chart_svg::{self, ChartSpec};
use super::super::html::html_escape;
use crate::error::{McpDomainError, McpDomainResult};
use serde::de::DeserializeOwned;
use systemprompt_models::artifacts::dashboard::{
    ChartSectionData, DashboardSection, LayoutWidth, ListSectionData, MetricStatus,
    MetricsCardsData, SectionType, StatusSectionData, TableSectionData, TextSectionData,
    TimelineSectionData,
};
use systemprompt_models::artifacts::types::ChartType;

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
        r#"<div class="dashboard-section" id="{id}" style="flex-basis: {width}">
    <h2 class="section-title">{title}</h2>
    <div class="section-content">
        {content}
    </div>
</div>"#,
        id = html_escape(section.section_id.as_str()),
        width = width_basis(section.layout.width),
        title = html_escape(&section.title),
        content = content,
    ))
}

const fn width_basis(width: LayoutWidth) -> &'static str {
    match width {
        LayoutWidth::Full => "100%",
        LayoutWidth::Half => "50%",
        LayoutWidth::Third => "33.333%",
        LayoutWidth::TwoThirds => "66.666%",
    }
}

fn section_data<T: DeserializeOwned>(section: &DashboardSection) -> McpDomainResult<T> {
    serde_json::from_value(section.data.clone()).map_err(|e| {
        McpDomainError::Internal(format!(
            "Dashboard section '{}' data does not match its declared type: {e}",
            section.section_id.as_str()
        ))
    })
}

fn render_metrics(section: &DashboardSection) -> McpDomainResult<String> {
    let data: MetricsCardsData = section_data(section)?;

    let cards = data
        .cards
        .iter()
        .map(|card| {
            let status_class = card.status.map_or("", |s| match s {
                MetricStatus::Success => " metric-status-success",
                MetricStatus::Warning => " metric-status-warning",
                MetricStatus::Error => " metric-status-error",
                MetricStatus::Info => " metric-status-info",
            });
            let icon = card.icon.as_ref().map_or_else(String::new, |i| {
                format!(r#"<span class="metric-icon">{}</span>"#, html_escape(i))
            });
            let subtitle = card.subtitle.as_ref().map_or_else(String::new, |s| {
                format!(r#"<div class="metric-subtitle">{}</div>"#, html_escape(s))
            });

            format!(
                r#"<div class="metric-card{status_class}">
                    <div class="metric-value">{icon}{value}</div>
                    <div class="metric-label">{label}</div>
                    {subtitle}
                </div>"#,
                value = html_escape(&card.value),
                label = html_escape(&card.title),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(r#"<div class="metrics-grid">{cards}</div>"#))
}

fn render_chart(section: &DashboardSection) -> McpDomainResult<String> {
    let data: ChartSectionData = section_data(section)?;
    let spec = ChartSpec {
        chart_type: chart_type(&data.chart_type),
        labels: &data.labels,
        datasets: &data.datasets,
        x_axis_label: "",
        y_axis_label: "",
    };

    Ok(format!(
        r#"<div class="chart-container">{}</div>"#,
        chart_svg::render(&spec, &section.title)
    ))
}

// Why: a dashboard section carries its chart kind as a free string rather than
// the `ChartType` enum, so an unknown value has to degrade to a plot rather
// than fail the whole dashboard.
fn chart_type(declared: &str) -> ChartType {
    match declared.to_lowercase().as_str() {
        "bar" | "column" => ChartType::Bar,
        "pie" => ChartType::Pie,
        "doughnut" | "donut" => ChartType::Doughnut,
        "area" => ChartType::Area,
        _ => ChartType::Line,
    }
}

fn render_table(section: &DashboardSection) -> McpDomainResult<String> {
    let data: TableSectionData = section_data(section)?;

    let header = data.columns.iter().fold(String::new(), |mut acc, c| {
        acc.push_str(&format!("<th>{}</th>", html_escape(c)));
        acc
    });

    let body = data.rows.iter().fold(String::new(), |mut acc, row| {
        let cells = row.as_object().map_or_else(
            || {
                row.as_array().map_or_else(Vec::new, |arr| {
                    arr.iter().map(ToString::to_string).collect()
                })
            },
            |obj| {
                data.columns
                    .iter()
                    .map(|c| {
                        obj.get(c.as_str())
                            .map_or_else(String::new, ToString::to_string)
                    })
                    .collect::<Vec<_>>()
            },
        );

        let cells_html = cells.iter().fold(String::new(), |mut acc, c| {
            acc.push_str(&format!("<td>{}</td>", html_escape(c.trim_matches('"'))));
            acc
        });

        acc.push_str(&format!("<tr>{cells_html}</tr>"));
        acc
    });

    Ok(format!(
        r#"<div class="section-table-wrap">
                <table class="section-table">
                    <thead><tr>{header}</tr></thead>
                    <tbody>{body}</tbody>
                </table>
            </div>"#,
    ))
}

fn render_status(section: &DashboardSection) -> McpDomainResult<String> {
    let data: StatusSectionData = section_data(section)?;

    let mut items = data
        .services
        .iter()
        .map(|service| {
            let status_class = match service.status.to_lowercase().as_str() {
                "ok" | "healthy" | "success" | "active" | "running" => "status-ok",
                "warning" | "degraded" => "status-warning",
                "error" | "failed" | "critical" => "status-error",
                _ => "status-unknown",
            };
            let uptime = service.uptime.as_ref().map_or_else(String::new, |u| {
                format!(r#"<span class="status-uptime">{}</span>"#, html_escape(u))
            });

            format!(
                r#"<div class="status-item">
                    <span class="status-indicator {status_class}"></span>
                    <span class="status-name">{name}</span>
                    <span class="status-value">{status}</span>
                    {uptime}
                </div>"#,
                name = html_escape(&service.name),
                status = html_escape(&service.status),
            )
        })
        .collect::<Vec<_>>();

    if let Some(db) = &data.database {
        items.push(format!(
            r#"<div class="status-item">
                <span class="status-indicator status-ok"></span>
                <span class="status-name">Database</span>
                <span class="status-value">{status} ({size:.1} MB)</span>
            </div>"#,
            status = html_escape(&db.status),
            size = db.size_mb,
        ));
    }

    if let Some(errors) = &data.recent_errors {
        items.push(format!(
            r#"<div class="status-item">
                <span class="status-indicator status-warning"></span>
                <span class="status-name">Recent errors</span>
                <span class="status-value">{critical} critical, {error} error, {warn} warn</span>
            </div>"#,
            critical = errors.critical,
            error = errors.error,
            warn = errors.warn,
        ));
    }

    Ok(format!(
        r#"<div class="status-list">{}</div>"#,
        items.join("\n")
    ))
}

fn render_list(section: &DashboardSection) -> McpDomainResult<String> {
    let data: ListSectionData = section_data(section)?;

    let lists = data
        .lists
        .iter()
        .map(|list| {
            let items = list
                .items
                .iter()
                .map(|item| {
                    let badge = item.badge.as_ref().map_or_else(String::new, |b| {
                        format!(r#"<span class="list-badge">{}</span>"#, html_escape(b))
                    });
                    format!(
                        r#"<li><span class="list-rank">{rank}</span><span class="list-label">{label}</span><span class="list-value">{value}</span>{badge}</li>"#,
                        rank = item.rank,
                        label = html_escape(&item.label),
                        value = html_escape(&item.value),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                r#"<div class="list-group">
                    <h3 class="list-group-title">{title}</h3>
                    <ul class="section-list">{items}</ul>
                </div>"#,
                title = html_escape(&list.title),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(lists)
}

fn render_timeline(section: &DashboardSection) -> McpDomainResult<String> {
    let data: TimelineSectionData = section_data(section)?;

    let events = data
        .events
        .iter()
        .map(|event| {
            let description = event.description.as_ref().map_or_else(String::new, |d| {
                format!(
                    r#"<div class="timeline-description">{}</div>"#,
                    html_escape(d)
                )
            });
            format!(
                r#"<li class="timeline-event">
                    <span class="timeline-timestamp">{timestamp}</span>
                    <span class="timeline-label">{label}</span>
                    {description}
                </li>"#,
                timestamp = html_escape(&event.timestamp),
                label = html_escape(&event.label),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(r#"<ol class="timeline">{events}</ol>"#))
}

fn render_text(section: &DashboardSection) -> McpDomainResult<String> {
    let data: TextSectionData = section_data(section)?;
    Ok(format!(
        r#"<p class="section-text">{}</p>"#,
        html_escape(&data.text)
    ))
}
