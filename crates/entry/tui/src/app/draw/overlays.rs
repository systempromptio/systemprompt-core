use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::components::chat::{
    render_input_request, render_task_detail, render_tool_panel, should_show_input_request,
    should_show_task_detail, should_show_tool_panel,
};
use crate::components::render_approval_dialog;
use crate::config::TuiConfig;
use crate::layout::AppLayout;
use crate::state::AppState;

pub fn render_overlays(
    frame: &mut Frame,
    layout: &AppLayout,
    state: &AppState,
    config: &TuiConfig,
) {
    if state.init_status.is_initializing {
        render_init_overlay(frame, layout.full_area, state, config);
        return;
    }

    if state.has_pending_approval() {
        render_approval_dialog(frame, layout.full_area, state, config);
    }

    if should_show_tool_panel(state) {
        render_tool_panel(frame, state, config);
    }

    if should_show_input_request(state) {
        render_input_request(frame, state, config);
    }

    if should_show_task_detail(state) {
        render_task_detail(frame, state, config);
    }
}

fn render_init_overlay(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let overlay_width = 50.min(area.width.saturating_sub(4));
    let overlay_height = 8.min(area.height.saturating_sub(4));
    let overlay_x = (area.width.saturating_sub(overlay_width)) / 2;
    let overlay_y = (area.height.saturating_sub(overlay_height)) / 2;

    let overlay_area = Rect {
        x: area.x + overlay_x,
        y: area.y + overlay_y,
        width: overlay_width,
        height: overlay_height,
    };

    frame.render_widget(Clear, overlay_area);

    let orange = config.theme.brand_primary;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(orange))
        .title(" Initializing ")
        .title_style(Style::default().fg(orange).bold());

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let progress = if state.init_status.total_steps > 0 {
        state.init_status.steps_completed as f64 / state.init_status.total_steps as f64
    } else {
        0.0
    };
    let bar_width = inner.width.saturating_sub(4) as usize;
    let filled = (progress * bar_width as f64) as usize;
    let empty = bar_width.saturating_sub(filled);
    let progress_bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            &state.init_status.current_step,
            Style::default().fg(Color::White),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            progress_bar,
            Style::default().fg(orange),
        )]),
        Line::from(vec![Span::styled(
            format!(
                "{}/{}",
                state.init_status.steps_completed, state.init_status.total_steps
            ),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}
