use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

use super::content_pane_block;
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_conversations(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = content_pane_block(config);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.conversations.conversations.is_empty() {
        let empty_msg = Paragraph::new("No conversations found. Press 'n' to create one.")
            .style(Style::default().fg(Color::DarkGray).italic())
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(inner);

    let table_area = chunks[0];
    let help_area = chunks[1];

    let header_cells = ["Name", "Messages", "Tasks", "Last Activity"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().bold().fg(Color::Cyan)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = state
        .conversations
        .conversations
        .iter()
        .enumerate()
        .map(|(idx, conv)| {
            let is_editing =
                state.conversations.editing && idx == state.conversations.selected_index;
            let last_activity = format_relative_time(conv.last_message_at);

            let name_cell = if is_editing {
                let edit_text = format!("{}|", state.conversations.edit_buffer);
                Cell::from(edit_text).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Cell::from(truncate_string(&conv.name, 30)).style(Style::default())
            };

            Row::new(vec![
                name_cell,
                Cell::from(conv.message_count.to_string()).style(Style::default()),
                Cell::from(conv.task_count.to_string()).style(Style::default()),
                Cell::from(last_activity).style(Style::default().fg(Color::DarkGray)),
            ])
        });

    let widths = [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut table_state = TableState::default();
    table_state.select(Some(state.conversations.selected_index));

    frame.render_stateful_widget(table, table_area, &mut table_state);

    render_help_line(frame, help_area, state);
}

fn render_help_line(frame: &mut Frame, area: Rect, state: &AppState) {
    let help_text = if state.conversations.editing {
        "Type to edit | Enter: save | Esc: cancel"
    } else {
        "j/k: navigate | Enter: select | e: edit | d: delete | n: new | r: refresh"
    };

    let line = Line::from(vec![Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray).italic(),
    )]);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn format_relative_time(dt: Option<chrono::DateTime<Utc>>) -> String {
    let Some(dt) = dt else {
        return "Never".to_string();
    };

    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        "Just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_days() < 7 {
        format!("{}d ago", duration.num_days())
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}
