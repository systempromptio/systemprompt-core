use ratatui::prelude::*;
use systemprompt_models::a2a::{Message, Part, Task, TaskState};

use crate::config::TuiConfig;
use crate::state::{format_duration, short_id, InlineToolCall, ToolCallStatus};

pub fn format_a2a_message(message: &Message, config: &TuiConfig) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let (role_text, role_color) = match message.role.as_str() {
        "user" => ("You: ", config.theme.user_message),
        "agent" => ("Assistant: ", config.theme.assistant_message),
        _ => ("System: ", Color::Yellow),
    };

    let mut first_line = true;

    for part in &message.parts {
        match part {
            Part::Text(text_part) => {
                let mut prev_empty = false;
                for (i, line) in text_part.text.lines().enumerate() {
                    let is_empty = line.trim().is_empty();
                    if is_empty && prev_empty {
                        continue;
                    }
                    prev_empty = is_empty;

                    if first_line && i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled(
                                role_text.to_string(),
                                Style::default().fg(role_color).bold(),
                            ),
                            Span::raw(line.to_string()),
                        ]));
                        first_line = false;
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::raw(line.to_string()),
                        ]));
                    }
                }
            },
            Part::File(file_part) => {
                let name = file_part.file.name.as_deref().unwrap_or("file");
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled("📎 ", Style::default()),
                    Span::styled(name.to_string(), Style::default().fg(Color::Cyan)),
                ]));
            },
            Part::Data(_) => {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled("📊 ", Style::default()),
                    Span::styled("[data]", Style::default().fg(Color::Magenta)),
                ]));
            },
        }
    }

    if first_line {
        lines.push(Line::from(vec![
            Span::styled(
                role_text.to_string(),
                Style::default().fg(role_color).bold(),
            ),
            Span::styled("(empty)", Style::default().fg(Color::DarkGray).italic()),
        ]));
    }

    lines
}

pub fn render_inline_tool_call(tool: &InlineToolCall, config: &TuiConfig) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let (status_icon, status_color) = match tool.status {
        ToolCallStatus::Pending | ToolCallStatus::Executing => ("..", Color::Yellow),
        ToolCallStatus::Approved | ToolCallStatus::Completed => ("ok", Color::Green),
        ToolCallStatus::Rejected | ToolCallStatus::Failed => ("!!", Color::Red),
    };

    lines.push(Line::from(vec![
        Span::styled("  > ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            tool.name.clone(),
            Style::default().fg(config.theme.tool_call).bold(),
        ),
        Span::styled(
            format!(" [{}]", status_icon),
            Style::default().fg(status_color),
        ),
    ]));

    if let Some(ref result) = tool.result_preview {
        let preview = if result.len() > 60 {
            format!("{}...", &result[..57])
        } else {
            result.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(preview, Style::default().fg(Color::DarkGray).italic()),
        ]));
    }

    lines
}

pub fn render_task_footer(task: &Task, _config: &TuiConfig) -> Vec<Line<'static>> {
    let mut spans = vec![Span::styled("  ", Style::default())];

    let (state_icon, state_text, state_color) = match task.status.state {
        TaskState::Completed => ("✓", "Completed", Color::Green),
        TaskState::Failed => ("✗", "Failed", Color::Red),
        TaskState::Canceled | TaskState::Rejected => ("⊘", "Canceled", Color::Yellow),
        TaskState::Working => ("◉", "Working", Color::Yellow),
        TaskState::Pending | TaskState::Submitted => ("○", "Pending", Color::DarkGray),
        TaskState::InputRequired | TaskState::AuthRequired => {
            ("?", "Input Required", Color::Magenta)
        },
        TaskState::Unknown => ("?", "Unknown", Color::DarkGray),
    };

    spans.push(Span::styled(
        format!("{} {}", state_icon, state_text),
        Style::default().fg(state_color),
    ));

    if let Some(ref metadata) = task.metadata {
        spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!("⚡ {}", metadata.agent_name),
            Style::default().fg(Color::Cyan),
        ));

        if let Some(ref model) = metadata.model {
            spans.push(Span::styled(
                format!(" ({})", model),
                Style::default().fg(Color::DarkGray),
            ));
        }

        if let Some(execution_time_ms) = metadata.execution_time_ms {
            spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format_duration(execution_time_ms),
                Style::default().fg(Color::Cyan),
            ));
        }

        if let Some(ref steps) = metadata.execution_steps {
            if !steps.is_empty() {
                spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(
                    format!("{} steps", steps.len()),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        if let (Some(i), Some(o)) = (metadata.input_tokens, metadata.output_tokens) {
            spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!("{}→{} tok", i, o),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled(
        short_id(task.id.as_ref()),
        Style::default().fg(Color::DarkGray),
    ));

    vec![Line::from(spans)]
}

pub fn render_pending_message(content: &str, config: &TuiConfig) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let first_line = content.lines().next().unwrap_or("");
    lines.push(Line::from(vec![
        Span::styled(
            "You: ".to_string(),
            Style::default().fg(config.theme.user_message).bold(),
        ),
        Span::raw(first_line.to_string()),
    ]));

    let mut prev_empty = first_line.trim().is_empty();
    for line in content.lines().skip(1) {
        let is_empty = line.trim().is_empty();
        if is_empty && prev_empty {
            continue;
        }
        prev_empty = is_empty;

        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::raw(line.to_string()),
        ]));
    }

    lines
}
