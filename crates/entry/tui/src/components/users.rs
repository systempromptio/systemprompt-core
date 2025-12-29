use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use systemprompt_models::BaseRoles;

use super::content_pane_block;
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_users(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = content_pane_block(config);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.users.users.is_empty() {
        let empty_msg = Paragraph::new("No users found")
            .style(Style::default().fg(Color::DarkGray).italic())
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(inner);

    let table_area = chunks[0];
    let selector_area = chunks[1];

    let header_cells = ["Name", "UUID", "Sessions", "Last Active", "Roles"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().bold().fg(Color::Cyan)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = state.users.users.iter().map(|user| {
        let last_active = format_relative_time(user.last_accessed);

        let base_style = if user.is_system() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let role_style = if user.is_system() {
            Style::default().fg(Color::DarkGray)
        } else if user.is_admin() {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let name_display = if user.is_system() {
            format!("{} [locked]", truncate_string(&user.name, 15))
        } else {
            truncate_string(&user.name, 20)
        };

        Row::new(vec![
            Cell::from(name_display).style(base_style),
            Cell::from(truncate_uuid(user.id.as_ref())).style(base_style),
            Cell::from(user.sessions.to_string()).style(base_style),
            Cell::from(last_active).style(base_style),
            Cell::from(user.role_display()).style(role_style),
        ])
    });

    let widths = [
        Constraint::Length(22),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Min(15),
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
    table_state.select(Some(state.users.selected_index));

    frame.render_stateful_widget(table, table_area, &mut table_state);

    render_role_selector(frame, selector_area, state);
}

fn render_role_selector(frame: &mut Frame, area: Rect, state: &AppState) {
    let selected_user = state.users.selected_user();
    let can_edit = state.users.can_edit_selected();

    let mut spans = vec![Span::styled(
        "Roles: ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    for (idx, role) in BaseRoles::available_roles().iter().enumerate() {
        let is_selected_role = idx == state.users.selected_role_index;
        let has_role = selected_user.is_some_and(|u| u.has_role(role));

        let role_text = if has_role {
            format!("[x] {} ", role)
        } else {
            format!("[ ] {} ", role)
        };

        let style = if !can_edit {
            Style::default().fg(Color::DarkGray)
        } else if is_selected_role {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if has_role {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        spans.push(Span::styled(role_text, style));
    }

    let help_text = if can_edit {
        "  (h/l: select role, space: toggle)"
    } else {
        "  (system user - read only)"
    };
    spans.push(Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray).italic(),
    ));

    let line = Line::from(spans);
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

fn truncate_uuid(uuid: &str) -> String {
    if uuid.len() >= 8 {
        format!("{}...", &uuid[..8])
    } else {
        uuid.to_string()
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
