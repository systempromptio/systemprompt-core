//! Metric, chart, status, list, timeline and text section bodies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::super::chart_svg::{self, ChartSpec};
use super::super::super::html::html_escape;
use super::{section_data, section_empty, status_class};
use crate::error::McpDomainResult;
use systemprompt_models::artifacts::dashboard::{
    ChartSectionData, DashboardSection, ListSectionData, MetricStatus, MetricsCardsData,
    StatusSectionData, TextSectionData, TimelineSectionData,
};
use systemprompt_models::artifacts::types::ChartType;

pub(super) fn render_metrics(section: &DashboardSection) -> McpDomainResult<String> {
    let data: MetricsCardsData = section_data(section)?;

    if data.cards.is_empty() {
        return Ok(section_empty("No metrics to show."));
    }

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

pub(super) fn render_chart(section: &DashboardSection) -> McpDomainResult<String> {
    let data: ChartSectionData = section_data(section)?;
    let spec = ChartSpec {
        chart_type: chart_type(&data.chart_type),
        labels: &data.labels,
        datasets: &data.datasets,
        x_axis_label: &data.x_axis_label,
        y_axis_label: &data.y_axis_label,
        y_axis_type: data.y_axis_type,
    };

    Ok(format!(
        r#"<div class="chart-container">{}</div>"#,
        chart_svg::render(&spec, &section.title)
    ))
}

fn chart_type(declared: &str) -> ChartType {
    match declared.to_lowercase().as_str() {
        "bar" | "column" => ChartType::Bar,
        "pie" => ChartType::Pie,
        "doughnut" | "donut" => ChartType::Doughnut,
        "area" => ChartType::Area,
        _ => ChartType::Line,
    }
}

pub(super) fn render_status(section: &DashboardSection) -> McpDomainResult<String> {
    let data: StatusSectionData = section_data(section)?;

    let mut items = data
        .services
        .iter()
        .map(|service| {
            let status_class = status_class(&service.status);
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
                <span class="status-indicator {status_class}"></span>
                <span class="status-name">Database</span>
                <span class="status-value">{status} ({size:.1} MB)</span>
            </div>"#,
            status_class = status_class(&db.status),
            status = html_escape(&db.status),
            size = db.size_mb,
        ));
    }

    if let Some(errors) = &data.recent_errors {
        items.push(format!(
            r#"<div class="status-item">
                <span class="status-indicator {status_class}"></span>
                <span class="status-name">Recent errors</span>
                <span class="status-value">{critical} critical, {error} error, {warn} warn</span>
            </div>"#,
            status_class = if errors.critical > 0 || errors.error > 0 {
                "status-error"
            } else if errors.warn > 0 {
                "status-warning"
            } else {
                "status-ok"
            },
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

pub(super) fn render_list(section: &DashboardSection) -> McpDomainResult<String> {
    let data: ListSectionData = section_data(section)?;

    if data.lists.is_empty() {
        return Ok(section_empty("Nothing to show."));
    }

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

pub(super) fn render_timeline(section: &DashboardSection) -> McpDomainResult<String> {
    let data: TimelineSectionData = section_data(section)?;

    if data.events.is_empty() {
        return Ok(section_empty("No events to show."));
    }

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

pub(super) fn render_text(section: &DashboardSection) -> McpDomainResult<String> {
    let data: TextSectionData = section_data(section)?;
    Ok(format!(
        r#"<p class="section-text">{}</p>"#,
        html_escape(&data.text)
    ))
}
