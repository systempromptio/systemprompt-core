use ratatui::prelude::*;
use systemprompt_models::AgentCard;

use crate::state::{AgentDisplayMetadata, SystemInstructionsSource};

pub fn build_system_instructions_section(
    card: &AgentCard,
    metadata: Option<&AgentDisplayMetadata>,
    expanded: bool,
) -> Vec<Line<'static>> {
    let system_prompt = card
        .capabilities
        .extensions
        .as_ref()
        .and_then(|exts| {
            exts.iter()
                .find(|e| e.uri == "systemprompt:system-instructions")
        })
        .and_then(|ext| ext.params.as_ref())
        .and_then(|p| p.get("systemPrompt"))
        .and_then(|v| v.as_str());

    let Some(prompt) = system_prompt else {
        return Vec::new();
    };

    let char_count = prompt.len();
    let mut lines = vec![];

    let expand_hint = if expanded { "[-]" } else { "[i]" };
    lines.push(Line::from(vec![
        Span::styled(
            "System Instructions ",
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::styled(
            format!("{} ({} chars)", expand_hint, char_count),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    if let Some(meta) = metadata {
        let source_display = match &meta.system_instructions_source {
            SystemInstructionsSource::FilePath(p) => shorten_path(p),
            SystemInstructionsSource::Inline => "(inline)".to_string(),
            SystemInstructionsSource::Unknown => "(unknown)".to_string(),
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", source_display),
            Style::default().fg(Color::Blue).italic(),
        )]));
    }

    if expanded {
        for line in prompt.lines().take(50) {
            lines.push(Line::from(vec![Span::styled(
                format!("  {}", line),
                Style::default().fg(Color::White),
            )]));
        }
        if prompt.lines().count() > 50 {
            lines.push(Line::from(vec![Span::styled(
                "  ... (truncated)",
                Style::default().fg(Color::DarkGray).italic(),
            )]));
        }
    } else {
        let preview: String = prompt.chars().take(100).collect();
        let preview = preview.replace('\n', " ");
        lines.push(Line::from(vec![Span::styled(
            format!("  {}...", preview),
            Style::default().fg(Color::DarkGray).italic(),
        )]));
    }

    lines.push(Line::from(""));
    lines
}

pub fn shorten_path(path: &std::path::Path) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let display = path.display().to_string();
    if !home.is_empty() && display.starts_with(&home) {
        format!("~{}", &display[home.len()..])
    } else {
        display
    }
}
