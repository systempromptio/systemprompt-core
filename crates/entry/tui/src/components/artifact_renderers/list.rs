use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
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
        lines.push(Line::from(""));
    }

    let items = data.get("items").and_then(|v| v.as_array());

    if let Some(items) = items {
        let is_ordered = data
            .get("ordered")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        for (idx, item) in items.iter().enumerate() {
            let bullet = if is_ordered {
                format!("{}. ", idx + 1)
            } else {
                "• ".to_string()
            };

            if let Some(item_str) = item.as_str() {
                lines.push(Line::from(vec![
                    Span::styled(bullet, Style::default().fg(Color::DarkGray)),
                    Span::raw(item_str.to_string()),
                ]));
            } else if let Some(item_obj) = item.as_object() {
                let title = item_obj.get("title").and_then(|v| v.as_str());
                let summary = item_obj.get("summary").and_then(|v| v.as_str());
                let link = item_obj.get("link").and_then(|v| v.as_str());

                if let Some(title) = title {
                    let mut spans = vec![
                        Span::styled(bullet.clone(), Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            title.to_string(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ];

                    if link.is_some() {
                        spans.push(Span::styled(" 🔗", Style::default().fg(Color::Blue)));
                    }

                    lines.push(Line::from(spans));

                    if let Some(summary) = summary {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(summary.to_string(), Style::default().fg(Color::DarkGray)),
                        ]));
                    }

                    if let Some(link) = link {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                link.to_string(),
                                Style::default().fg(Color::Blue).italic(),
                            ),
                        ]));
                    }

                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled(bullet, Style::default().fg(Color::DarkGray)),
                        Span::raw(item.to_string()),
                    ]));
                }
            } else {
                lines.push(Line::from(vec![
                    Span::styled(bullet, Style::default().fg(Color::DarkGray)),
                    Span::raw(item.to_string()),
                ]));
            }
        }
    } else {
        lines.push(Line::from("No items"));
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
            "List: {}",
            artifact.name.as_deref().unwrap_or("Unnamed")
        )),
        Line::from(""),
        Line::from("No data available"),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}
