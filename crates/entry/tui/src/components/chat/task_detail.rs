use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::config::TuiConfig;
use crate::state::{format_duration, short_id, AppState, TaskState};
use systemprompt_models::a2a::{Message as A2aMessage, Part, Task, TaskMetadata};
use systemprompt_models::execution::{ExecutionStep, StepStatus};

pub fn render_task_detail(frame: &mut Frame, state: &AppState, config: &TuiConfig) {
    let Some(task) = state.chat.selected_task() else {
        return;
    };

    let area = centered_rect(70, 80, frame.area());
    frame.render_widget(Clear, area);

    let lines = build_task_content(task, config);
    let task_state = task.status.state;

    let (state_icon, state_color) = task_state_display(task_state);

    let task_num = state
        .chat
        .selected_task_index
        .map(|i| format!("{}/{}", i + 1, state.chat.tasks.len()))
        .unwrap_or_default();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_focused))
        .title(format!(" Task {} ", short_id(task.id.as_ref())))
        .title_bottom(Line::from(vec![
            Span::raw(" "),
            Span::styled(state_icon, Style::default().fg(state_color).bold()),
            Span::raw(" | "),
            Span::styled(task_num, Style::default().fg(Color::Cyan)),
            Span::raw(" | [←→] nav [↑↓] scroll [Del] delete [q] close "),
        ]));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.chat.task_detail_scroll as u16, 0));

    frame.render_widget(paragraph, inner);
}

const fn task_state_display(state: TaskState) -> (&'static str, Color) {
    match state {
        TaskState::Completed => ("Completed", Color::Green),
        TaskState::Failed => ("Failed", Color::Red),
        TaskState::Canceled | TaskState::Rejected => ("Canceled", Color::Yellow),
        TaskState::Working => ("Working", Color::Yellow),
        _ => ("Pending", Color::DarkGray),
    }
}

fn build_task_content(task: &Task, config: &TuiConfig) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    append_task_header(&mut lines, task);
    if let Some(ref metadata) = task.metadata {
        append_task_metadata(&mut lines, metadata, config);
    }
    if let Some(ref history) = task.history {
        append_task_history(&mut lines, history, config);
    }
    if let Some(ref metadata) = task.metadata {
        if let Some(ref steps) = metadata.execution_steps {
            append_execution_steps(&mut lines, steps, config);
        }
    }
    lines
}

fn append_task_header(lines: &mut Vec<Line<'static>>, task: &Task) {
    lines.push(Line::from(vec![
        Span::styled("Task ID: ", Style::default().fg(Color::Gray)),
        Span::styled(
            task.id.as_ref().to_string(),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Context: ", Style::default().fg(Color::Gray)),
        Span::styled(
            short_id(task.context_id.as_ref()),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));
}

fn append_task_metadata(
    lines: &mut Vec<Line<'static>>,
    metadata: &TaskMetadata,
    config: &TuiConfig,
) {
    lines.push(Line::from(Span::styled(
        "Metadata",
        Style::default().fg(config.theme.border_focused).bold(),
    )));
    lines.push(Line::from(Span::styled(
        "─".repeat(50),
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(vec![
        Span::styled("  Agent: ", Style::default().fg(Color::Gray)),
        Span::styled(
            metadata.agent_name.clone(),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    if let Some(ref model) = metadata.model {
        lines.push(Line::from(vec![
            Span::styled("  Model: ", Style::default().fg(Color::Gray)),
            Span::styled(model.clone(), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(ms) = metadata.execution_time_ms {
        lines.push(Line::from(vec![
            Span::styled("  Duration: ", Style::default().fg(Color::Gray)),
            Span::styled(format_duration(ms), Style::default().fg(Color::Cyan)),
        ]));
    }

    if let (Some(input), Some(output)) = (metadata.input_tokens, metadata.output_tokens) {
        lines.push(Line::from(vec![
            Span::styled("  Tokens: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{} in / {} out", input, output),
                Style::default().fg(Color::White),
            ),
        ]));
    }

    lines.push(Line::from(""));
}

fn append_task_history(lines: &mut Vec<Line<'static>>, history: &[A2aMessage], config: &TuiConfig) {
    for msg in history {
        let (role_label, role_color) = match msg.role.as_str() {
            "user" => ("You", config.theme.user_message),
            "agent" => ("Assistant", config.theme.assistant_message),
            _ => ("System", Color::Yellow),
        };

        lines.push(Line::from(Span::styled(
            role_label.to_string(),
            Style::default().fg(role_color).bold(),
        )));
        lines.push(Line::from(Span::styled(
            "─".repeat(50),
            Style::default().fg(Color::DarkGray),
        )));

        for part in &msg.parts {
            append_message_part(lines, part);
        }
        lines.push(Line::from(""));
    }
}

fn append_message_part(lines: &mut Vec<Line<'static>>, part: &Part) {
    match part {
        Part::Text(text_part) => {
            for line in text_part.text.lines() {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::raw(line.to_string()),
                ]));
            }
        },
        Part::File(_) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("[file attachment]", Style::default().fg(Color::Cyan)),
            ]));
        },
        Part::Data(_) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("[data]", Style::default().fg(Color::Magenta)),
            ]));
        },
    }
}

fn append_execution_steps(
    lines: &mut Vec<Line<'static>>,
    steps: &[ExecutionStep],
    config: &TuiConfig,
) {
    if steps.is_empty() {
        return;
    }

    lines.push(Line::from(Span::styled(
        format!("Execution Steps ({})", steps.len()),
        Style::default().fg(config.theme.border_focused).bold(),
    )));
    lines.push(Line::from(Span::styled(
        "─".repeat(50),
        Style::default().fg(Color::DarkGray),
    )));

    for (i, step) in steps.iter().enumerate() {
        append_single_step(lines, step, i);
    }
}

fn append_single_step(lines: &mut Vec<Line<'static>>, step: &ExecutionStep, index: usize) {
    let (status_icon, status_color) = step_status_display(step.status);
    let step_type = step.step_type();
    let duration = step
        .duration_ms
        .map(|ms| format!(" ({}ms)", ms))
        .unwrap_or_default();

    lines.push(Line::from(vec![
        Span::styled(
            format!("  {} ", status_icon),
            Style::default().fg(status_color),
        ),
        Span::styled(
            format!("{}. ", index + 1),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(step_type.to_string(), Style::default().fg(Color::Cyan)),
        Span::styled(duration, Style::default().fg(Color::DarkGray)),
    ]));

    if let Some(tool_name) = step.tool_name() {
        lines.push(Line::from(vec![
            Span::styled("     Tool: ", Style::default().fg(Color::Gray)),
            Span::styled(tool_name.to_string(), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(result) = step.tool_result() {
        append_step_result(lines, result);
    }
}

const fn step_status_display(status: StepStatus) -> (&'static str, Color) {
    match status {
        StepStatus::Completed => ("✓", Color::Green),
        StepStatus::Failed => ("✗", Color::Red),
        StepStatus::InProgress => ("◉", Color::Yellow),
        StepStatus::Pending => ("○", Color::DarkGray),
    }
}

fn append_step_result(lines: &mut Vec<Line<'static>>, result: &serde_json::Value) {
    let result_str = serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string());
    let preview = if result_str.len() > 200 {
        format!("{}...", &result_str[..200])
    } else {
        result_str
    };
    for line in preview.lines().take(3) {
        lines.push(Line::from(vec![
            Span::styled("     ", Style::default()),
            Span::styled(
                line.to_string(),
                Style::default().fg(Color::DarkGray).italic(),
            ),
        ]));
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn should_show_task_detail(state: &AppState) -> bool {
    state.chat.has_task_selected() && state.chat.input_buffer.is_empty()
}
