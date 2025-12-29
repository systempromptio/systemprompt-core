use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use systemprompt_models::a2a::Artifact;

use super::text::extract_data_content;

pub fn render(artifact: &Artifact, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
    let Some(data) = extract_data_content(&artifact.parts) else {
        render_no_data(artifact, area, buf);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();

    if let Some(title) = data.get("title").and_then(|v| v.as_str()) {
        lines.push(Line::from(Span::styled(
            title.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
    }

    if let Some(subtitle) = data.get("subtitle").and_then(|v| v.as_str()) {
        lines.push(Line::from(Span::styled(
            subtitle.to_string(),
            Style::default().fg(Color::DarkGray).italic(),
        )));
    }

    if !lines.is_empty() {
        lines.push(Line::from(""));
    }

    if let Some(sections) = data.get("sections").and_then(|v| v.as_array()) {
        for section in sections {
            if let Some(section_obj) = section.as_object() {
                if let Some(heading) = section_obj.get("heading").and_then(|v| v.as_str()) {
                    let icon = section_obj
                        .get("icon")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    lines.push(Line::from(vec![
                        Span::raw(if icon.is_empty() {
                            String::new()
                        } else {
                            format!("{} ", icon)
                        }),
                        Span::styled(
                            heading.to_string(),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }

                if let Some(content) = section_obj.get("content").and_then(|v| v.as_str()) {
                    for line in content.lines() {
                        lines.push(Line::from(format!("  {}", line)));
                    }
                }

                if let Some(items) = section_obj.get("items").and_then(|v| v.as_array()) {
                    for item in items {
                        if let Some(item_str) = item.as_str() {
                            lines.push(Line::from(format!("  • {}", item_str)));
                        }
                    }
                }

                lines.push(Line::from(""));
            }
        }
    }

    if let Some(ctas) = data.get("ctas").and_then(|v| v.as_array()) {
        if !ctas.is_empty() {
            lines.push(Line::from(Span::styled(
                "Actions:",
                Style::default().fg(Color::DarkGray),
            )));
            for cta in ctas {
                if let Some(cta_obj) = cta.as_object() {
                    let label = cta_obj.get("label").and_then(|v| v.as_str()).unwrap_or("");
                    let url = cta_obj.get("url").and_then(|v| v.as_str());
                    if let Some(url) = url {
                        lines.push(Line::from(vec![
                            Span::raw("  → "),
                            Span::styled(label.to_string(), Style::default().fg(Color::Blue)),
                            Span::styled(
                                format!(" ({})", url),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(format!("  → {}", label)));
                    }
                }
            }
            lines.push(Line::from(""));
        }
    }

    if let Some(sources) = data.get("sources").and_then(|v| v.as_array()) {
        if !sources.is_empty() {
            lines.push(Line::from(Span::styled(
                "Sources:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for source in sources {
                if let Some(source_obj) = source.as_object() {
                    let title = source_obj
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Untitled");
                    let uri = source_obj.get("uri").and_then(|v| v.as_str());
                    let relevance = source_obj
                        .get("relevance")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0);

                    let relevance_bar = format_relevance(relevance);

                    lines.push(Line::from(vec![
                        Span::styled(relevance_bar, Style::default().fg(Color::Green)),
                        Span::raw(" "),
                        Span::styled(
                            title.to_string(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]));

                    if let Some(uri) = uri {
                        lines.push(Line::from(vec![
                            Span::raw("     "),
                            Span::styled(
                                uri.to_string(),
                                Style::default().fg(Color::Blue).italic(),
                            ),
                        ]));
                    }
                }
            }
        }
    }

    let scroll_y = u16::try_from(scroll_offset).unwrap_or(u16::MAX);
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    paragraph.render(area, buf);
}

fn render_no_data(artifact: &Artifact, area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::from(format!(
            "Card: {}",
            artifact.name.as_deref().unwrap_or("Unnamed")
        )),
        Line::from(""),
        Line::from("No data available"),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}

fn format_relevance(score: f64) -> String {
    let scaled = (score.clamp(0.0, 1.0) * 5.0).round();
    let filled = usize::try_from(scaled as i64).unwrap_or(0).min(5);
    let empty = 5 - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}
