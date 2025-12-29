use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::config::TuiConfig;
use crate::state::{AppState, InlineToolCall, ToolCallStatus};

pub fn render_tool_panel(frame: &mut Frame, state: &AppState, config: &TuiConfig) {
    let Some(tool) = state.chat.selected_tool() else {
        return;
    };

    let area = centered_rect(60, 70, frame.area());

    frame.render_widget(Clear, area);

    let lines = build_tool_content(tool, state.chat.tool_panel_scroll, config);

    let status_str = match tool.status {
        ToolCallStatus::Pending => "Pending",
        ToolCallStatus::Approved => "Approved",
        ToolCallStatus::Rejected => "Rejected",
        ToolCallStatus::Executing => "Executing...",
        ToolCallStatus::Completed => "Completed",
        ToolCallStatus::Failed => "Failed",
    };

    let status_color = match tool.status {
        ToolCallStatus::Pending => Color::Yellow,
        ToolCallStatus::Approved | ToolCallStatus::Completed => Color::Green,
        ToolCallStatus::Rejected | ToolCallStatus::Failed => Color::Red,
        ToolCallStatus::Executing => Color::Cyan,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_focused))
        .title(format!(" Tool: {} ", tool.name))
        .title_bottom(Line::from(vec![
            Span::raw(" Status: "),
            Span::styled(status_str, Style::default().fg(status_color).bold()),
            Span::raw(" | [↑↓] scroll [q] close "),
        ]));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.chat.tool_panel_scroll as u16, 0));

    frame.render_widget(paragraph, inner);
}

fn build_tool_content(
    tool: &InlineToolCall,
    _scroll: usize,
    config: &TuiConfig,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::Gray)),
            Span::styled(tool.id.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Arguments:",
            Style::default().fg(config.theme.border_focused).bold(),
        )]),
        Line::from(vec![Span::styled(
            "─".repeat(40),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let args_str = serde_json::to_string_pretty(&tool.arguments)
        .unwrap_or_else(|_| tool.arguments.to_string());

    lines.extend(args_str.lines().map(|line| {
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(line.to_string(), Style::default().fg(Color::Cyan)),
        ])
    }));

    lines.extend([
        Line::from(""),
        Line::from(vec![Span::styled(
            "Result:",
            Style::default().fg(config.theme.border_focused).bold(),
        )]),
        Line::from(vec![Span::styled(
            "─".repeat(40),
            Style::default().fg(Color::DarkGray),
        )]),
    ]);

    match (&tool.result, tool.status) {
        (Some(result), _) => {
            let content = serde_json::from_str::<serde_json::Value>(result)
                .ok()
                .and_then(|json| serde_json::to_string_pretty(&json).ok())
                .unwrap_or_else(|| result.clone());

            lines.extend(content.lines().map(|line| {
                Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(line.to_string(), Style::default().fg(Color::White)),
                ])
            }));
        },
        (None, ToolCallStatus::Executing) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("Executing...", Style::default().fg(Color::Yellow).italic()),
            ]));
        },
        (None, ToolCallStatus::Failed) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "(Error - no result)",
                    Style::default().fg(Color::Red).italic(),
                ),
            ]));
        },
        (None, _) => {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("(No result yet)", Style::default().fg(Color::Gray).italic()),
            ]));
        },
    }

    lines
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

pub const fn should_show_tool_panel(state: &AppState) -> bool {
    state.chat.selected_tool_index.is_some()
}
