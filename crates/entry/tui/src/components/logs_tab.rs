use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::config::TuiConfig;
use crate::messages::LogLevel;
use crate::state::AppState;

pub fn render_logs_tab(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    render_status_bar(frame, chunks[0], state, config);
    render_log_entries(frame, chunks[1], state, config);
}

fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let mut spans = vec![Span::styled(
        " Filter: ",
        Style::default().fg(Color::DarkGray),
    )];

    match &state.logs.filter_level {
        Some(level) => {
            let (text, color) = match level {
                LogLevel::Error => ("ERR", config.theme.log_error),
                LogLevel::Warn => ("WRN", config.theme.log_warn),
                LogLevel::Info => ("INF", config.theme.log_info),
                LogLevel::Debug | LogLevel::Trace => ("DBG", config.theme.log_debug),
            };
            spans.push(Span::styled(text, Style::default().fg(color).bold()));
        },
        None => {
            spans.push(Span::styled("ALL", Style::default().fg(Color::White)));
        },
    }

    spans.push(Span::raw("  "));

    if state.logs.follow_tail {
        spans.push(Span::styled(
            "FOLLOW",
            Style::default().fg(Color::Green).bold(),
        ));
    } else {
        spans.push(Span::styled("PAUSED", Style::default().fg(Color::DarkGray)));
    }

    let count = state.logs.filtered_entries().count();
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("{} entries", count),
        Style::default().fg(Color::DarkGray),
    ));

    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        "[e]rror [w]arn [i]nfo [a]ll [f]ollow",
        Style::default().fg(Color::DarkGray),
    ));

    let status_line = Paragraph::new(Line::from(spans));
    frame.render_widget(status_line, area);
}

fn render_log_entries(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let entries: Vec<_> = state.logs.filtered_entries().collect();

    if entries.is_empty() {
        let placeholder =
            Paragraph::new(" No log entries").style(Style::default().fg(Color::DarkGray).italic());
        frame.render_widget(placeholder, area);
        return;
    }

    let visible_count = usize::from(area.height);
    let total = entries.len();
    let start = total.saturating_sub(visible_count);

    let lines: Vec<Line> = entries
        .iter()
        .skip(start)
        .take(visible_count)
        .map(|entry| format_log_line(entry, config, usize::from(area.width)))
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn format_log_line<'a>(
    entry: &crate::messages::LogEntry,
    config: &TuiConfig,
    max_width: usize,
) -> Line<'a> {
    let timestamp = entry.timestamp.format("%H:%M:%S").to_string();

    let level_color = match entry.level {
        LogLevel::Error => config.theme.log_error,
        LogLevel::Warn => config.theme.log_warn,
        LogLevel::Info => config.theme.log_info,
        LogLevel::Debug | LogLevel::Trace => config.theme.log_debug,
    };

    let level_text = match entry.level {
        LogLevel::Error => "ERR",
        LogLevel::Warn => "WRN",
        LogLevel::Info => "INF",
        LogLevel::Debug | LogLevel::Trace => "DBG",
    };

    let prefix_len = 8 + 1 + 5 + 1 + entry.module.len().min(20) + 2;
    let max_message_width = max_width.saturating_sub(prefix_len);

    let module = if entry.module.len() > 20 {
        format!("{}...", &entry.module[..17])
    } else {
        entry.module.clone()
    };

    let message = if entry.message.len() > max_message_width && max_message_width > 3 {
        format!("{}...", &entry.message[..max_message_width - 3])
    } else {
        entry.message.clone()
    };

    Line::from(vec![
        Span::styled(timestamp, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled(
            format!("[{}]", level_text),
            Style::default().fg(level_color).bold(),
        ),
        Span::raw(" "),
        Span::styled(format!("{}:", module), Style::default().fg(Color::Cyan)),
        Span::raw(" "),
        Span::raw(message),
    ])
}
