use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::config::TuiConfig;
use crate::state::{AppState, InputType};

pub fn render_input_request(frame: &mut Frame, state: &AppState, config: &TuiConfig) {
    let Some(request) = state.chat.pending_input() else {
        return;
    };

    let area = centered_rect(50, 40, frame.area());

    frame.render_widget(Clear, area);

    let lines = build_input_content(request, config);

    let title = match request.input_type {
        InputType::Text => " Agent Input Request ",
        InputType::Choice => " Select an Option ",
        InputType::Confirm => " Confirm ",
    };

    let help_text = match request.input_type {
        InputType::Text => " [Enter] submit [Esc] cancel ",
        InputType::Choice | InputType::Confirm => " [↑↓] select [Enter] submit [Esc] cancel ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_focused))
        .title(title)
        .title_bottom(Line::from(help_text));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

    frame.render_widget(paragraph, inner);
}

fn build_input_content(
    request: &crate::state::InputRequest,
    config: &TuiConfig,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        request.prompt.clone(),
        Style::default().fg(Color::White).bold(),
    )]));

    lines.push(Line::from(""));

    match request.input_type {
        InputType::Text => {
            let input_display = if request.text_input.is_empty() {
                request
                    .default_value
                    .clone()
                    .map(|d| format!("(default: {})", d))
                    .unwrap_or_default()
            } else {
                request.text_input.clone()
            };

            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(config.theme.border_focused)),
                Span::styled(input_display, Style::default().fg(Color::Cyan)),
                Span::styled("█", Style::default().fg(Color::White)),
            ]));
        },
        InputType::Choice => {
            if let Some(choices) = &request.choices {
                for (i, choice) in choices.iter().enumerate() {
                    let is_selected = i == request.selected_choice;
                    let prefix = if is_selected { "● " } else { "○ " };
                    let style = if is_selected {
                        Style::default().fg(config.theme.border_focused).bold()
                    } else {
                        Style::default().fg(Color::Gray)
                    };

                    lines.push(Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled(choice.clone(), style),
                    ]));
                }
            }
        },
        InputType::Confirm => {
            let yes_selected = request.selected_choice == 0;
            let no_selected = request.selected_choice == 1;

            let yes_style = if yes_selected {
                Style::default().fg(Color::Green).bold()
            } else {
                Style::default().fg(Color::Gray)
            };
            let no_style = if no_selected {
                Style::default().fg(Color::Red).bold()
            } else {
                Style::default().fg(Color::Gray)
            };

            lines.push(Line::from(vec![
                Span::styled(if yes_selected { "● " } else { "○ " }, yes_style),
                Span::styled("Yes", yes_style),
            ]));
            lines.push(Line::from(vec![
                Span::styled(if no_selected { "● " } else { "○ " }, no_style),
                Span::styled("No", no_style),
            ]));
        },
    }

    lines
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub const fn should_show_input_request(state: &AppState) -> bool {
    state.chat.has_pending_input()
}
