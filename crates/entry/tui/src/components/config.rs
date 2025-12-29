use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

use super::content_pane_block;
use crate::config::TuiConfig;
use crate::state::{AppState, TuiModeInfo};

pub fn render_config(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = content_pane_block(config);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = build_mode_info_lines(&state.mode_info);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn build_mode_info_lines(mode_info: &TuiModeInfo) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    match mode_info {
        TuiModeInfo::Cloud {
            cloud_api_url,
            user_email,
            tenant_id,
            profile,
        } => {
            lines.push(Line::from(vec![
                Span::styled("Mode: ", Style::default().fg(Color::Cyan).bold()),
                Span::styled("Cloud", Style::default().fg(Color::Green).bold()),
            ]));
            lines.push(Line::from(Span::styled(
                "─".repeat(50),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));

            add_section_header(&mut lines, "Profile");
            add_config_line(&mut lines, "Name", &profile.display_name);
            add_config_line(&mut lines, "ID", &profile.name);
            lines.push(Line::from(""));

            add_section_header(&mut lines, "Server");
            add_config_line(&mut lines, "Host", &profile.server.host);
            add_config_line(&mut lines, "Port", &profile.server.port.to_string());
            add_config_line(&mut lines, "API URL", &profile.server.api_external_url);
            lines.push(Line::from(""));

            add_section_header(&mut lines, "Database");
            add_config_line(&mut lines, "Type", &profile.database.db_type);
            lines.push(Line::from(""));

            add_section_header(&mut lines, "Cloud");
            add_config_line(&mut lines, "API", cloud_api_url);
            if let Some(email) = user_email {
                add_config_line(&mut lines, "User", email);
            }
            if let Some(tenant) = tenant_id {
                add_config_line(&mut lines, "Tenant", tenant);
            }
            add_config_line_bool(&mut lines, "Connected", true);
        },
    }

    lines
}

fn add_section_header(lines: &mut Vec<Line<'static>>, title: &str) {
    lines.push(Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(Color::Cyan).bold(),
    )));
}

fn add_config_line(lines: &mut Vec<Line<'static>>, label: &str, value: &str) {
    lines.push(Line::from(vec![
        Span::styled(format!("  {}: ", label), Style::default().fg(Color::Yellow)),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ]));
}

fn add_config_line_bool(lines: &mut Vec<Line<'static>>, label: &str, value: bool) {
    let (text, color) = if value {
        ("yes", Color::Green)
    } else {
        ("no", Color::DarkGray)
    };
    lines.push(Line::from(vec![
        Span::styled(format!("  {}: ", label), Style::default().fg(Color::Yellow)),
        Span::styled(text.to_string(), Style::default().fg(color)),
    ]));
}
