use ratatui::prelude::*;
use ratatui::widgets::{Cell, Paragraph, Row, Table};

use super::tables::format_number;
use crate::components::inner_panel_block;
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_bot_traffic(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = inner_panel_block(config, false, " Bot Traffic ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let bot = &state.analytics.traffic_data.bot_traffic;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(inner);

    let items = [
        ("Total", format_number(bot.total_requests), Color::White),
        ("Human", format_number(bot.human_requests), Color::Green),
        ("Bot", format_number(bot.bot_requests), Color::Yellow),
        ("Bot %", format!("{:.1}%", bot.bot_percentage), Color::Cyan),
    ];

    for (i, (label, value, color)) in items.iter().enumerate() {
        let text = vec![
            Line::from(Span::styled(*label, Style::default().bold())),
            Line::from(Span::styled(value.clone(), Style::default().fg(*color))),
        ];
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, cols[i]);
    }
}

pub fn render_browser_stats(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = inner_panel_block(config, false, " Browsers ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.analytics.traffic_data.browsers.is_empty() {
        let no_data =
            Paragraph::new("No browser data available").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, inner);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Browser").style(Style::default().bold()),
        Cell::from("Count").style(Style::default().bold()),
        Cell::from("%").style(Style::default().bold()),
    ])
    .height(1);

    let rows: Vec<Row> = state
        .analytics
        .traffic_data
        .browsers
        .iter()
        .take(10)
        .map(|stat| {
            Row::new(vec![
                Cell::from(stat.browser.clone()),
                Cell::from(format_number(stat.count)),
                Cell::from(format!("{:.1}%", stat.percentage)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header);

    frame.render_widget(table, inner);
}

pub fn render_device_stats(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = inner_panel_block(config, false, " Devices ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.analytics.traffic_data.devices.is_empty() {
        let no_data =
            Paragraph::new("No device data available").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, inner);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Device").style(Style::default().bold()),
        Cell::from("Count").style(Style::default().bold()),
        Cell::from("%").style(Style::default().bold()),
    ])
    .height(1);

    let rows: Vec<Row> = state
        .analytics
        .traffic_data
        .devices
        .iter()
        .take(10)
        .map(|stat| {
            Row::new(vec![
                Cell::from(stat.device_type.clone()),
                Cell::from(format_number(stat.count)),
                Cell::from(format!("{:.1}%", stat.percentage)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header);

    frame.render_widget(table, inner);
}

pub fn render_geo_stats(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = inner_panel_block(config, false, " Countries ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.analytics.traffic_data.countries.is_empty() {
        let no_data = Paragraph::new("No geographic data available")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(no_data, inner);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Country").style(Style::default().bold()),
        Cell::from("Count").style(Style::default().bold()),
        Cell::from("%").style(Style::default().bold()),
    ])
    .height(1);

    let rows: Vec<Row> = state
        .analytics
        .traffic_data
        .countries
        .iter()
        .take(10)
        .map(|stat| {
            Row::new(vec![
                Cell::from(stat.country.clone()),
                Cell::from(format_number(stat.count)),
                Cell::from(format!("{:.1}%", stat.percentage)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header);

    frame.render_widget(table, inner);
}
