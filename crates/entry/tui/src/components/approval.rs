use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::config::TuiConfig;
use crate::layout::centered_rect;
use crate::state::{AppState, ApprovalAction, PendingApproval};
use crate::tools::RiskLevel;

pub fn render_approval_dialog(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let Some(pending) = state.tools.current_pending() else {
        return;
    };

    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_focused))
        .title(" Tool Approval Required ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(inner);

    render_tool_info(frame, chunks[0], pending, config);
    render_description(frame, chunks[1], pending);
    render_arguments(frame, chunks[2], pending);
    render_action_buttons(frame, chunks[3], pending);
}

fn render_tool_info(frame: &mut Frame, area: Rect, pending: &PendingApproval, config: &TuiConfig) {
    let risk_symbol = pending.tool_call.risk_level.symbol();
    let risk_label = pending.tool_call.risk_level.label();
    let risk_color = risk_level_color(pending.tool_call.risk_level);

    let tool_info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Tool: ", Style::default().bold()),
            Span::styled(
                pending.tool_call.tool_name.clone(),
                Style::default().fg(config.theme.tool_call),
            ),
        ]),
        Line::from(vec![
            Span::styled("Risk: ", Style::default().bold()),
            Span::styled(
                format!("{} {}", risk_symbol, risk_label),
                Style::default().fg(risk_color),
            ),
        ]),
    ]);
    frame.render_widget(tool_info, area);
}

fn render_description(frame: &mut Frame, area: Rect, pending: &PendingApproval) {
    let description = Paragraph::new(pending.tool_call.description.clone())
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    frame.render_widget(description, area);
}

fn render_arguments(frame: &mut Frame, area: Rect, pending: &PendingApproval) {
    let args_json = serde_json::to_string_pretty(&pending.tool_call.arguments)
        .unwrap_or_else(|_| "{}".to_string());

    let args_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Arguments ");

    let args_paragraph = Paragraph::new(args_json)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true })
        .block(args_block);

    frame.render_widget(args_paragraph, area);
}

fn render_action_buttons(frame: &mut Frame, area: Rect, pending: &PendingApproval) {
    let approve_style = action_button_style(
        pending.selected_action,
        ApprovalAction::Approve,
        Color::Green,
    );
    let reject_style =
        action_button_style(pending.selected_action, ApprovalAction::Reject, Color::Red);
    let edit_style =
        action_button_style(pending.selected_action, ApprovalAction::Edit, Color::Yellow);

    let buttons = Paragraph::new(Line::from(vec![
        Span::styled(" [Y]es ", approve_style),
        Span::raw("   "),
        Span::styled(" [N]o ", reject_style),
        Span::raw("   "),
        Span::styled(" [E]dit ", edit_style),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(buttons, area);
}

const fn risk_level_color(level: RiskLevel) -> Color {
    match level {
        RiskLevel::Safe => Color::Green,
        RiskLevel::Moderate => Color::Yellow,
        RiskLevel::Dangerous => Color::Red,
    }
}

fn action_button_style(selected: ApprovalAction, action: ApprovalAction, color: Color) -> Style {
    if selected == action {
        Style::default().fg(Color::Black).bg(color).bold()
    } else {
        Style::default().fg(color)
    }
}
