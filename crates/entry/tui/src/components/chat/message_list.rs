use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::message_formatting::{
    format_a2a_message, render_inline_tool_call, render_pending_message, render_task_footer,
};
use crate::components::spinner::{get_animated_dots, get_processing_spinner, get_spinner_frame};
use crate::config::TuiConfig;
use crate::state::{AppState, LoadingState};

pub fn render_chat_messages(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for task in &state.chat.tasks {
        if let Some(history) = &task.history {
            for message in history {
                lines.extend(format_a2a_message(message, config));
            }
        }
        lines.extend(render_task_footer(task, config));
    }

    if let Some(pending) = &state.chat.pending_user_message {
        lines.extend(render_pending_message(pending, config));
    }

    if state.chat.is_processing() {
        lines.extend(render_processing_content(state, config));
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Start a conversation by typing a message below.",
            Style::default().fg(Color::DarkGray).italic(),
        )]));
    }

    let area_width = usize::from(inner_area.width.saturating_sub(1).max(1));
    let visible_height = usize::from(inner_area.height);

    let mut total_visual_lines: usize = 0;
    for line in &lines {
        if line.spans.is_empty() {
            total_visual_lines += 1;
            continue;
        }

        let line_width: usize = line
            .spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum();

        if line_width == 0 {
            total_visual_lines += 1;
        } else {
            total_visual_lines += line_width.div_ceil(area_width);
        }
    }

    let scroll_offset = total_visual_lines.saturating_sub(visible_height);
    let scroll_y = u16::try_from(scroll_offset).unwrap_or(u16::MAX);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner_area);
}

fn render_processing_content(state: &AppState, config: &TuiConfig) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let loading = state.chat.loading_state();
    let spinner = get_spinner_frame();

    let status_line = match loading {
        LoadingState::Idle => return lines,

        LoadingState::Sending => {
            let (proc_spinner, spinner_color) = get_processing_spinner();
            let dots = get_animated_dots();
            Line::from(vec![
                Span::styled(
                    format!("{} ", proc_spinner),
                    Style::default().fg(spinner_color),
                ),
                Span::styled(
                    "Processing",
                    Style::default().fg(Color::Rgb(255, 165, 0)).italic(),
                ),
                Span::styled(dots.to_string(), Style::default().fg(Color::White)),
            ])
        },

        LoadingState::Connecting => Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Cyan)),
            Span::styled(
                "Connecting to agent...",
                Style::default().fg(Color::Cyan).italic(),
            ),
        ]),

        LoadingState::Streaming => {
            let step_count = state.chat.current_step_count();
            let elapsed = state
                .chat
                .streaming_started_at()
                .map_or(0, |t| (chrono::Utc::now() - t).num_seconds());

            let mut spans = vec![
                Span::styled(format!("{} ", spinner), Style::default().fg(Color::Green)),
                Span::styled("Generating", Style::default().fg(Color::Green).italic()),
            ];

            if elapsed > 0 {
                spans.push(Span::styled(
                    format!(" {}s", elapsed),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if step_count > 0 {
                spans.push(Span::styled(
                    format!(" · {} steps", step_count),
                    Style::default().fg(Color::Cyan),
                ));
            }

            Line::from(spans)
        },

        LoadingState::WaitingForTool => Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Magenta)),
            Span::styled(
                "Executing step...",
                Style::default().fg(Color::Magenta).italic(),
            ),
        ]),

        LoadingState::WaitingForInput => Line::from(vec![
            Span::styled("? ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "Waiting for input...",
                Style::default().fg(Color::Yellow).italic(),
            ),
        ]),
    };

    lines.push(Line::from(vec![Span::styled(
        "Assistant: ",
        Style::default().fg(config.theme.assistant_message).bold(),
    )]));

    let mut status_spans = vec![Span::styled("  ", Style::default())];
    status_spans.extend(status_line.spans);
    lines.push(Line::from(status_spans));

    for tool in &state.chat.current_inline_tools {
        lines.extend(render_inline_tool_call(tool, config));
    }

    let response = state.chat.streaming_response();
    if !response.is_empty() {
        lines.push(Line::from(""));
        for line in response.lines() {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::raw(line.to_string()),
            ]));
        }
    }

    lines
}
