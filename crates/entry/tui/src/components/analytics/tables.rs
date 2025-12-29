use ratatui::prelude::*;
use ratatui::widgets::{Cell, Paragraph, Row, Table};

use crate::components::inner_panel_block;
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_content_stats(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = inner_panel_block(config, false, " Content Stats ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.analytics.content_stats.is_empty() {
        let no_data =
            Paragraph::new("No content data available").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, inner);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Type").style(Style::default().bold()),
        Cell::from("Count").style(Style::default().bold().fg(Color::Cyan)),
        Cell::from("Size").style(Style::default().bold()),
    ])
    .height(1);

    let rows: Vec<Row> = state
        .analytics
        .content_stats
        .iter()
        .map(|stat| {
            let content_type = if stat.content_type.len() > 30 {
                format!("{}...", &stat.content_type[..27])
            } else {
                stat.content_type.clone()
            };
            let size = stat
                .total_size
                .map_or_else(|| "-".to_string(), format_number);
            Row::new(vec![
                Cell::from(content_type),
                Cell::from(format_number(stat.count)).style(Style::default().fg(Color::Cyan)),
                Cell::from(size),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(30),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(table, inner);
}

pub fn render_recent_conversations(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    config: &TuiConfig,
) {
    let block = inner_panel_block(config, false, " Recent Conversations ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.analytics.recent_conversations.is_empty() {
        let no_data =
            Paragraph::new("No recent conversations").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, inner);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Agent").style(Style::default().bold()),
        Cell::from("User").style(Style::default().bold()),
        Cell::from("Msgs").style(Style::default().bold()),
        Cell::from("Last Activity").style(Style::default().bold()),
    ])
    .height(1);

    let now = chrono::Utc::now();
    let rows: Vec<Row> = state
        .analytics
        .recent_conversations
        .iter()
        .map(|conv| {
            let agent = conv.agent_name.as_ref().map_or_else(
                || "-".to_string(),
                |name| {
                    if name.len() > 15 {
                        format!("{}...", &name[..12])
                    } else {
                        name.clone()
                    }
                },
            );
            let user = conv.user_name.as_ref().map_or_else(
                || "-".to_string(),
                |name| {
                    if name.len() > 15 {
                        format!("{}...", &name[..12])
                    } else {
                        name.clone()
                    }
                },
            );

            let time_ago = format_time_ago(now, conv.last_activity);

            Row::new(vec![
                Cell::from(agent),
                Cell::from(user),
                Cell::from(conv.message_count.to_string()),
                Cell::from(time_ago),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(15),
            Constraint::Length(15),
            Constraint::Length(6),
            Constraint::Min(12),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(table, inner);
}

pub fn format_number(n: i64) -> String {
    let abs_n = n.unsigned_abs();
    let sign = if n < 0 { "-" } else { "" };

    if abs_n >= 1_000_000 {
        format!(
            "{sign}{}.{}M",
            abs_n / 1_000_000,
            (abs_n % 1_000_000) / 100_000
        )
    } else if abs_n >= 1_000 {
        format!("{sign}{}.{}k", abs_n / 1_000, (abs_n % 1_000) / 100)
    } else {
        n.to_string()
    }
}

fn format_time_ago(
    now: chrono::DateTime<chrono::Utc>,
    then: chrono::DateTime<chrono::Utc>,
) -> String {
    let duration = now.signed_duration_since(then);

    if duration.num_days() > 0 {
        format!("{}d ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{}m ago", duration.num_minutes())
    } else {
        "just now".to_string()
    }
}
