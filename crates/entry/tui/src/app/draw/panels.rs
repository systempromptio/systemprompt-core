use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::components::{control_guide_width, render_control_guide};
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_with_control_guide<F>(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    config: &TuiConfig,
    render_content: F,
) where
    F: FnOnce(&mut Frame, Rect),
{
    let guide_width = control_guide_width();
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(guide_width)])
        .split(area);

    render_content(frame, horizontal_chunks[0]);
    render_control_guide(frame, horizontal_chunks[1], state.active_tab, config);
}

pub fn render_chat_input(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    config: &TuiConfig,
) -> Position {
    let running_services = state
        .services
        .services
        .iter()
        .filter(|s| matches!(s.status, crate::state::RuntimeStatus::Running))
        .count();
    let total_services = state.services.services.len();

    let streaming_indicator = if state.chat.is_processing() {
        " [Processing...] "
    } else {
        ""
    };

    let pending_tools = state.tools.pending_approvals.len();
    let tools_indicator = if pending_tools > 0 {
        format!(" [{}] ", pending_tools)
    } else {
        String::new()
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.brand_primary))
        .title(Span::styled(
            " Input ",
            Style::default().fg(config.theme.brand_primary).bold(),
        ))
        .title_alignment(Alignment::Left);

    let inner_area = input_block.inner(area);
    frame.render_widget(input_block, area);

    let right_content = format!(
        "{}{}Services: {}/{}",
        streaming_indicator, tools_indicator, running_services, total_services
    );
    let right_len = right_content.len();
    let separator_len = 3;
    let available_input_width = inner_area.width as usize - right_len - separator_len - 2;

    let (input_display, input_style) = if state.chat.input_buffer.is_empty() {
        (
            "Type a message... (/help for commands)".to_string(),
            Style::default().fg(Color::DarkGray).italic(),
        )
    } else {
        let input = &state.chat.input_buffer;
        let display = if input.len() > available_input_width {
            format!("…{}", &input[input.len() - available_input_width + 1..])
        } else {
            input.clone()
        };
        (display, Style::default().fg(Color::White))
    };

    let input_padded = format!("{:<width$}", input_display, width = available_input_width);

    let full_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(input_padded, input_style),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(right_content, Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(full_line);
    frame.render_widget(paragraph, inner_area);

    let cursor_x =
        inner_area.x + 1 + state.chat.input_buffer.len().min(available_input_width) as u16;
    let cursor_y = inner_area.y;
    Position::new(cursor_x, cursor_y)
}
