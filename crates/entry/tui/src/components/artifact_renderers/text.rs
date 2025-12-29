use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use systemprompt_models::a2a::{Artifact, Part};

pub fn render(artifact: &Artifact, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
    let content = extract_text_content(&artifact.parts);
    let lines: Vec<Line> = content.lines().map(Line::from).collect();

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset as u16, 0));

    paragraph.render(area, buf);
}

pub fn render_fallback(artifact: &Artifact, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
    let content = extract_text_content(&artifact.parts);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &artifact.metadata.artifact_type,
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
    ];

    lines.extend(content.lines().map(Line::from));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset as u16, 0));

    paragraph.render(area, buf);
}

pub fn render_chart_fallback(artifact: &Artifact, area: Rect, buf: &mut Buffer) {
    let data = extract_data_content(&artifact.parts);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Chart: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                artifact.name.as_deref().unwrap_or("Unnamed"),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(data) = data {
        if let Some(chart_type) = data.get("chart_type").and_then(|v| v.as_str()) {
            lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
                Span::raw(chart_type),
            ]));
        }

        if let Some(labels) = data.get("labels").and_then(|v| v.as_array()) {
            let label_preview: Vec<String> = labels
                .iter()
                .take(5)
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let suffix = if labels.len() > 5 { "..." } else { "" };
            lines.push(Line::from(vec![
                Span::styled("Labels: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("[{}]{}", label_preview.join(", "), suffix)),
            ]));
        }

        if let Some(datasets) = data.get("datasets").and_then(|v| v.as_array()) {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Datasets:",
                Style::default().fg(Color::DarkGray),
            )));
            for dataset in datasets.iter().take(3) {
                if let Some(label) = dataset.get("label").and_then(|v| v.as_str()) {
                    let data_preview = dataset
                        .get("data")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            let nums: Vec<String> = arr
                                .iter()
                                .take(5)
                                .filter_map(|v| v.as_f64().map(|n| format!("{:.1}", n)))
                                .collect();
                            let suffix = if arr.len() > 5 { "..." } else { "" };
                            format!("[{}]{}", nums.join(", "), suffix)
                        })
                        .unwrap_or_default();
                    lines.push(Line::from(format!("  {} {}", label, data_preview)));
                }
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Chart visualization not available in terminal",
        Style::default().fg(Color::DarkGray).italic(),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}

pub fn extract_text_content(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            Part::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn extract_data_content(parts: &[Part]) -> Option<&serde_json::Map<String, serde_json::Value>> {
    parts.iter().find_map(|p| match p {
        Part::Data(d) => Some(&d.data),
        _ => None,
    })
}
