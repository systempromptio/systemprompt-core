mod charts;
mod tables;

use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Tabs};

use crate::components::{content_pane_block, inner_panel_block};
use crate::config::TuiConfig;
use crate::state::{AnalyticsView, AppState};

pub fn render_analytics(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = content_pane_block(config);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.analytics.loading && state.analytics.user_metrics.is_none() {
        let loading = Paragraph::new("Loading analytics data...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(loading, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(inner);

    render_view_tabs(frame, chunks[0], state, config);

    match state.analytics.active_view {
        AnalyticsView::Content => render_content_view(frame, chunks[1], state, config),
        AnalyticsView::Conversations => render_conversations_view(frame, chunks[1], state, config),
        AnalyticsView::Traffic => render_traffic_view(frame, chunks[1], state, config),
    }
}

fn render_view_tabs(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let titles: Vec<Line> = [
        AnalyticsView::Content,
        AnalyticsView::Conversations,
        AnalyticsView::Traffic,
    ]
    .iter()
    .enumerate()
    .map(|(i, view)| {
        let is_selected = state.analytics.active_view == *view;
        let style = if is_selected {
            Style::default().fg(config.theme.brand_primary).bold()
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Line::from(vec![
            Span::styled(
                format!("[{}] ", i + 1),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(view.label(), style),
        ])
    })
    .collect();

    let tabs = Tabs::new(titles)
        .style(Style::default())
        .highlight_style(Style::default().fg(config.theme.brand_primary))
        .select(match state.analytics.active_view {
            AnalyticsView::Content => 0,
            AnalyticsView::Conversations => 1,
            AnalyticsView::Traffic => 2,
        })
        .divider(" │ ");

    frame.render_widget(tabs, area);
}

fn render_content_view(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(area);

    render_user_metrics(frame, chunks[0], state, config);
    tables::render_content_stats(frame, chunks[1], state, config);
}

fn render_conversations_view(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(area);

    render_user_metrics(frame, chunks[0], state, config);
    tables::render_recent_conversations(frame, chunks[1], state, config);
}

fn render_traffic_view(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    charts::render_bot_traffic(frame, chunks[0], state, config);
    charts::render_browser_stats(frame, chunks[1], state, config);
    charts::render_device_stats(frame, chunks[2], state, config);
    charts::render_geo_stats(frame, chunks[3], state, config);
}

fn render_user_metrics(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = inner_panel_block(config, false, " User Metrics ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(metrics) = &state.analytics.user_metrics {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
                Constraint::Ratio(1, 4),
            ])
            .split(inner);

        render_metric_card(frame, cols[0], "Total", metrics.total_users);
        render_metric_card(frame, cols[1], "Active", metrics.active_users);
        render_metric_card_with_trend(
            frame,
            cols[2],
            "New (7d)",
            metrics.new_users_week,
            metrics.users_trend_7d,
        );
        render_metric_card_with_trend(
            frame,
            cols[3],
            "New (30d)",
            metrics.new_users_month,
            metrics.users_trend_30d,
        );
    } else {
        let no_data =
            Paragraph::new("No user metrics available").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, inner);
    }
}

fn render_metric_card(frame: &mut Frame, area: Rect, label: &str, value: i64) {
    let text = vec![Line::from(vec![
        Span::styled(label, Style::default().bold()),
        Span::raw(": "),
        Span::styled(
            tables::format_number(value),
            Style::default().fg(Color::White),
        ),
    ])];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_metric_card_with_trend(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    current: i64,
    trend_pct: f64,
) {
    let (trend_text, trend_color, trend_arrow) = if trend_pct > 0.0 {
        (format!("+{:.1}%", trend_pct), Color::Green, "↑")
    } else if trend_pct < 0.0 {
        (format!("{:.1}%", trend_pct), Color::Red, "↓")
    } else {
        ("0%".to_string(), Color::DarkGray, "→")
    };

    let text = vec![
        Line::from(vec![
            Span::styled(label, Style::default().bold()),
            Span::raw(": "),
            Span::styled(
                tables::format_number(current),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(trend_arrow, Style::default().fg(trend_color)),
            Span::raw(" "),
            Span::styled(trend_text, Style::default().fg(trend_color)),
        ]),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}
