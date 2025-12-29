use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::spinner::get_spinner_frame;
use crate::config::TuiConfig;
use crate::state::{AppState, InputMode};

pub fn render_global_input(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    config: &TuiConfig,
) -> Option<Position> {
    let mode_text = match state.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Insert => "INSERT",
        InputMode::Command => "COMMAND",
    };

    let mode_color = match state.input_mode {
        InputMode::Normal => Color::Blue,
        InputMode::Insert => Color::Green,
        InputMode::Command => Color::Yellow,
    };

    let running_services = state
        .services
        .services
        .iter()
        .filter(|s| matches!(s.status, crate::state::RuntimeStatus::Running))
        .count();
    let total_services = state.services.services.len();

    let (streaming_indicator, is_processing) = if state.chat.is_processing() {
        (format!(" {} ", get_spinner_frame()), true)
    } else {
        (String::new(), false)
    };

    let pending_tools = state.tools.pending_approvals.len();
    let tools_indicator = if pending_tools > 0 {
        format!(" [{}] ", pending_tools)
    } else {
        String::new()
    };

    let total_tokens: u32 = state
        .chat
        .tasks
        .iter()
        .filter_map(|task| {
            let metadata = task.metadata.as_ref()?;
            let input = metadata.input_tokens.unwrap_or(0);
            let output = metadata.output_tokens.unwrap_or(0);
            Some(input + output)
        })
        .sum();

    let tokens_indicator = if total_tokens > 0 {
        format!("Tokens: {} | ", format_tokens(total_tokens))
    } else {
        String::new()
    };

    let input_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(config.theme.border_unfocused))
        .title(Span::styled(
            format!(" {} ", mode_text),
            Style::default().fg(Color::Black).bg(mode_color).bold(),
        ))
        .title_alignment(Alignment::Left);

    let inner_area = input_block.inner(area);
    frame.render_widget(input_block, area);

    let right_content = format!(
        "{}{}{}Services: {}/{}",
        tokens_indicator, streaming_indicator, tools_indicator, running_services, total_services
    );
    let right_len = right_content.len();
    let separator_len = 3;
    let available_input_width = usize::from(inner_area.width) - right_len - separator_len - 2;

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

    let status_color = if is_processing {
        config.theme.brand_primary
    } else {
        Color::DarkGray
    };

    let full_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(input_padded, input_style),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(right_content, Style::default().fg(status_color)),
    ]);

    let paragraph = Paragraph::new(full_line);
    frame.render_widget(paragraph, inner_area);

    if matches!(state.input_mode, InputMode::Insert) {
        let cursor_x =
            inner_area.x + 1 + state.chat.input_buffer.len().min(available_input_width) as u16;
        let cursor_y = inner_area.y;
        Some(Position::new(cursor_x, cursor_y))
    } else {
        None
    }
}

fn format_tokens(tokens: u32) -> String {
    if tokens < 1000 {
        tokens.to_string()
    } else if tokens < 1_000_000 {
        format!("{:.1}k", f64::from(tokens) / 1000.0)
    } else {
        format!("{:.1}M", f64::from(tokens) / 1_000_000.0)
    }
}
