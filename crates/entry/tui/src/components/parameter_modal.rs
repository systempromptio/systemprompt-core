use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::cli_registry::CliArgType;
use crate::config::TuiConfig;
use crate::state::{AppState, ParameterModalState};

pub fn render_parameter_modal(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let Some(modal) = &state.commands.modal_state else {
        return;
    };

    let popup_area = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup_area);

    let title = format!(" {} ", modal.command.name);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_focused))
        .title(title)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if modal.command.arguments.is_empty() {
        render_no_args_content(frame, inner, modal);
        return;
    }

    render_form_content(frame, inner, modal, config);
}

fn render_no_args_content(frame: &mut Frame, area: Rect, modal: &ParameterModalState) {
    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            "This command has no parameters.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Command: systemprompt {}", modal.command.path.join(" ")),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to execute or Esc to cancel",
            Style::default().fg(Color::Cyan),
        )),
    ];

    let paragraph = Paragraph::new(content).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_form_content(
    frame: &mut Frame,
    area: Rect,
    modal: &ParameterModalState,
    config: &TuiConfig,
) {
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Fill in the parameters below. ",
            Style::default().fg(Color::Gray),
        ),
        Span::styled("*", Style::default().fg(Color::Red)),
        Span::styled(" = required", Style::default().fg(Color::Gray)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    let field_area = chunks[1];
    let field_height = 3u16;
    let max_fields = (field_area.height / field_height) as usize;

    for (i, arg) in modal.command.arguments.iter().take(max_fields).enumerate() {
        let y_offset = i as u16 * field_height;
        if y_offset + field_height > field_area.height {
            break;
        }

        let field_rect = Rect {
            x: field_area.x + 2,
            y: field_area.y + y_offset,
            width: field_area.width.saturating_sub(4),
            height: field_height,
        };

        let is_focused = i == modal.focused_field_index;
        let value = modal
            .field_values
            .get(arg.name.as_ref())
            .cloned()
            .unwrap_or_default();
        let has_error = modal.validation_errors.contains_key(arg.name.as_ref());

        let label_style = if arg.required {
            Style::default().fg(Color::Yellow).bold()
        } else {
            Style::default().fg(Color::Gray)
        };

        let required_marker = if arg.required {
            Span::styled("*", Style::default().fg(Color::Red))
        } else {
            Span::raw("")
        };

        let type_hint = match arg.arg_type {
            CliArgType::Bool => " (yes/no)",
            CliArgType::Number => " (number)",
            CliArgType::Path => " (path)",
            CliArgType::String => "",
        };

        let label_line = Line::from(vec![
            Span::styled(arg.name.as_ref(), label_style),
            required_marker,
            Span::styled(type_hint, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(
                format!("- {}", arg.help),
                Style::default().fg(Color::DarkGray).italic(),
            ),
        ]);

        let input_style = if is_focused {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else if has_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };

        let cursor = if is_focused { "█" } else { "" };
        let display_value = if is_focused {
            let (before, after) = value.split_at(modal.cursor_position.min(value.len()));
            format!("{before}{cursor}{after}")
        } else {
            value.clone()
        };

        let input_line = Line::from(vec![
            Span::styled("> ", Style::default().fg(config.theme.border_focused)),
            Span::styled(display_value, input_style),
        ]);

        let field_content = vec![label_line, input_line];
        let paragraph = Paragraph::new(field_content);
        frame.render_widget(paragraph, field_rect);
    }

    let footer_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" next field  "),
            Span::styled("Enter", Style::default().fg(Color::Green).bold()),
            Span::raw(" execute  "),
            Span::styled("Esc", Style::default().fg(Color::Red).bold()),
            Span::raw(" cancel"),
        ]),
    ];

    let footer = Paragraph::new(footer_lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    frame.render_widget(footer, chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
