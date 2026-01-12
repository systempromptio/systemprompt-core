use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use systemprompt_models::a2a::Artifact;
use systemprompt_models::artifacts::dashboard::MetricStatus;

use super::text::extract_data_content;

pub fn render(artifact: &Artifact, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
    let Some(data) = extract_data_content(&artifact.parts) else {
        render_no_data(artifact, area, buf);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    if let Some(title) = data.get("title").and_then(|v| v.as_str()) {
        lines.push(Line::from(Span::styled(
            format!("📊 {}", title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    if let Some(sections) = data.get("sections").and_then(|v| v.as_array()) {
        for section in sections {
            if let Some(section_obj) = section.as_object() {
                let section_type = section_obj
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                let section_title = section_obj.get("title").and_then(|v| v.as_str());

                let type_icon = match section_type {
                    "metrics" | "metric_cards" => "📈",
                    "table" => "📋",
                    "chart" => "📊",
                    "list" => "📝",
                    "timeline" => "⏱️",
                    "status" => "🔘",
                    _ => "▪",
                };

                if let Some(title) = section_title {
                    lines.push(Line::from(vec![
                        Span::raw(format!("{} ", type_icon)),
                        Span::styled(
                            title,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(" ({})", section_type),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }

                render_section_content(section_obj, section_type, &mut lines);

                lines.push(Line::from(""));
            }
        }
    }

    let scroll_y = u16::try_from(scroll_offset).unwrap_or(u16::MAX);
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    paragraph.render(area, buf);
}

fn render_section_content<'a>(
    section: &'a serde_json::Map<String, serde_json::Value>,
    section_type: &str,
    lines: &mut Vec<Line<'a>>,
) {
    match section_type {
        "metrics" | "metric_cards" => {
            if let Some(metrics) = section.get("data").and_then(|v| v.as_array()) {
                for metric in metrics {
                    if let Some(metric_obj) = metric.as_object() {
                        let label = metric_obj
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let value = metric_obj
                            .get("value")
                            .map_or_else(|| "-".to_string(), format_value);
                        let trend = metric_obj.get("trend").and_then(|v| v.as_str());

                        let trend_indicator = match trend {
                            Some("up") => Span::styled(" ↑", Style::default().fg(Color::Green)),
                            Some("down") => Span::styled(" ↓", Style::default().fg(Color::Red)),
                            _ => Span::raw(""),
                        };

                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(label, Style::default().fg(Color::DarkGray)),
                            Span::raw(": "),
                            Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
                            trend_indicator,
                        ]));
                    }
                }
            }
        },
        "table" => {
            if let Some(data) = section.get("data").and_then(|v| v.as_object()) {
                if let Some(columns) = data.get("columns").and_then(|v| v.as_array()) {
                    let headers: Vec<&str> = columns
                        .iter()
                        .filter_map(|c| c.get("label").and_then(|v| v.as_str()))
                        .collect();
                    lines.push(Line::from(format!("  Columns: {}", headers.join(" | "))));
                }
                if let Some(items) = data.get("items").and_then(|v| v.as_array()) {
                    lines.push(Line::from(format!("  {} rows", items.len())));
                }
            }
        },
        "list" => {
            if let Some(items) = section.get("data").and_then(|v| v.as_array()) {
                for (i, item) in items.iter().take(5).enumerate() {
                    if let Some(item_str) = item.as_str() {
                        lines.push(Line::from(format!("  {}. {}", i + 1, item_str)));
                    } else if let Some(item_obj) = item.as_object() {
                        let title = item_obj
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        lines.push(Line::from(format!("  {}. {}", i + 1, title)));
                    }
                }
                if items.len() > 5 {
                    lines.push(Line::from(Span::styled(
                        format!("  ... and {} more", items.len() - 5),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        },
        "status" => {
            if let Some(items) = section.get("data").and_then(|v| v.as_array()) {
                for item in items {
                    if let Some(item_obj) = item.as_object() {
                        let name = item_obj.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let status = item_obj
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        let status_color =
                            match status.parse::<MetricStatus>().unwrap_or(MetricStatus::Info) {
                                MetricStatus::Success => Color::Green,
                                MetricStatus::Warning => Color::Yellow,
                                MetricStatus::Error => Color::Red,
                                MetricStatus::Info => Color::DarkGray,
                            };

                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled("●", Style::default().fg(status_color)),
                            Span::raw(format!(" {} - ", name)),
                            Span::styled(status, Style::default().fg(status_color)),
                        ]));
                    }
                }
            }
        },
        "chart" => {
            let chart_type = section
                .get("data")
                .and_then(|v| v.get("chart_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            lines.push(Line::from(Span::styled(
                format!("  [Chart: {} - not available in terminal]", chart_type),
                Style::default().fg(Color::DarkGray).italic(),
            )));
        },
        _ => {
            if let Some(data) = section.get("data") {
                let preview = data.to_string();
                let truncated = if preview.len() > 100 {
                    format!("{}...", &preview[..100])
                } else {
                    preview
                };
                lines.push(Line::from(Span::styled(
                    format!("  {}", truncated),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        },
    }
}

fn render_no_data(artifact: &Artifact, area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::from(format!(
            "Dashboard: {}",
            artifact.name.as_deref().unwrap_or("Unnamed")
        )),
        Line::from(""),
        Line::from("No data available"),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}

fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.as_f64().map_or_else(
            || n.to_string(),
            |f| {
                if f.fract() == 0.0 && f.abs() < 1_000_000.0 {
                    let int_val = f.clamp(i64::MIN as f64, i64::MAX as f64) as i64;
                    format!("{}", int_val)
                } else if f.abs() >= 1_000_000.0 {
                    format!("{:.1}M", f / 1_000_000.0)
                } else if f.abs() >= 1_000.0 {
                    format!("{:.1}K", f / 1_000.0)
                } else {
                    format!("{:.2}", f)
                }
            },
        ),
        serde_json::Value::Bool(b) => if *b { "Yes" } else { "No" }.to_string(),
        _ => value.to_string(),
    }
}
