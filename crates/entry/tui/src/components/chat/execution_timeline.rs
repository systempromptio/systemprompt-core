use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::config::TuiConfig;
use crate::state::{format_duration, AppState, ExecutionStepDisplay, StepStatusDisplay};

pub fn render_execution_timeline(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    config: &TuiConfig,
) {
    if !state.chat.show_execution_timeline || state.chat.current_execution_steps().is_empty() {
        return;
    }

    let steps = state.chat.current_execution_steps();
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![Span::styled(
        format!(" Execution Timeline ({} steps) ", steps.len()),
        Style::default().fg(config.theme.border_focused).bold(),
    )]));
    lines.push(Line::from(""));

    for (i, step) in steps.iter().enumerate() {
        let is_last = i == steps.len() - 1;
        lines.extend(render_step(step, is_last, config));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_unfocused))
        .title(" Timeline ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_step(
    step: &ExecutionStepDisplay,
    is_last: bool,
    _config: &TuiConfig,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let (status_icon, status_color) = match step.status {
        StepStatusDisplay::Pending => ("○", Color::Gray),
        StepStatusDisplay::InProgress => ("◉", Color::Yellow),
        StepStatusDisplay::Completed => ("✓", Color::Green),
        StepStatusDisplay::Failed => ("✗", Color::Red),
    };

    let type_icon = match step.step_type.as_deref() {
        Some("plan" | "planning") => "📋",
        Some("execute" | "execution" | "tool_execution") => "🔧",
        Some("validate" | "validation") => "✔",
        Some("understanding") => "💭",
        Some("completion") => "✨",
        _ => "•",
    };

    let step_desc = match (&step.step_type, &step.tool_name) {
        (Some(t), Some(tool)) => format!("{}: {}", capitalize(t), tool),
        (Some(t), None) => capitalize(t),
        (None, Some(tool)) => format!("Tool: {}", tool),
        (None, None) => "Processing".to_string(),
    };

    let duration_str = step
        .duration_ms
        .map(|ms| format!(" [{}]", format_duration(i64::from(ms))))
        .unwrap_or_default();

    let connector = if is_last { "└─" } else { "├─" };

    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", connector),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{} ", status_icon),
            Style::default().fg(status_color),
        ),
        Span::styled(format!("{} ", type_icon), Style::default()),
        Span::styled(step_desc, Style::default().fg(Color::White)),
        Span::styled(duration_str, Style::default().fg(Color::Cyan)),
    ]));

    if let Some(content) = &step.content {
        let preview: String = content.chars().take(60).collect();
        let vertical_line = if is_last { "   " } else { " │ " };
        if !preview.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(vertical_line, Style::default().fg(Color::DarkGray)),
                Span::styled("  ", Style::default()),
                Span::styled(
                    if preview.len() >= 60 {
                        format!("{}...", preview)
                    } else {
                        preview
                    },
                    Style::default().fg(Color::DarkGray).italic(),
                ),
            ]));
        }
    }

    lines
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

pub fn timeline_height(state: &AppState) -> u16 {
    if !state.chat.show_execution_timeline || state.chat.current_execution_steps().is_empty() {
        return 0;
    }
    let steps = state.chat.current_execution_steps().len();
    let content_lines = steps * 2;
    (content_lines + 4).min(15) as u16
}
