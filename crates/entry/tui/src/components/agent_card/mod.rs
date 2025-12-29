mod metadata;
mod sections;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use systemprompt_models::AgentCard;

use crate::config::TuiConfig;
use crate::state::{AgentDisplayMetadata, AgentInfo, AppState, FocusedPanel};

use metadata::build_system_instructions_section;
use sections::{
    build_capabilities_section, build_description_section, build_docs_line, build_io_modes_line,
    build_mcp_servers_section, build_provider_line, build_security_section, build_skills_section,
    build_status_line,
};

pub fn render_agent_card(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let is_focused = state.focus == FocusedPanel::Sidebar;
    let border_color = if is_focused {
        config.theme.border_focused
    } else {
        config.theme.border_unfocused
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" Active Agent ");

    let block_inner = block.inner(area);
    frame.render_widget(block, area);

    let inner = Rect {
        x: block_inner.x.saturating_add(1),
        y: block_inner.y + 1,
        width: block_inner.width.saturating_sub(2),
        height: block_inner.height.saturating_sub(1),
    };

    let lines = build_agent_card_content(state);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn build_agent_card_content(state: &AppState) -> Vec<Line<'static>> {
    let agent_info = state.agents.get_selected_agent();
    let agent_card = state.agents.get_selected_card();
    let display_metadata = state.agents.get_selected_display_metadata();
    let instructions_expanded = state.agents.instructions_expanded;

    match (agent_info, agent_card) {
        (Some(info), Some(card)) => {
            build_full_agent_content(info, card, display_metadata, instructions_expanded)
        },
        (Some(info), None) => build_loading_agent_content(info),
        _ => build_no_agent_content(state),
    }
}

fn build_full_agent_content(
    info: &AgentInfo,
    card: &AgentCard,
    metadata: Option<&AgentDisplayMetadata>,
    instructions_expanded: bool,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("Name: ", Style::default().fg(Color::Cyan).bold()),
        Span::styled(card.name.clone(), Style::default().fg(Color::White).bold()),
        Span::styled(
            format!(" v{}", card.version),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    lines.extend(build_status_line(info));
    lines.extend(build_provider_line(card));
    lines.push(Line::from(""));
    lines.extend(build_description_section(card));
    lines.push(Line::from(""));
    lines.extend(build_capabilities_section(card));
    lines.extend(build_io_modes_line(card));
    lines.push(Line::from(""));
    lines.extend(build_security_section(card));
    lines.extend(build_skills_section(card, metadata));
    lines.extend(build_mcp_servers_section(card, metadata));
    lines.extend(build_system_instructions_section(
        card,
        metadata,
        instructions_expanded,
    ));
    lines.extend(build_docs_line(card));

    lines
}

fn build_loading_agent_content(info: &AgentInfo) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(
                info.display_name.clone(),
                Style::default().fg(Color::White).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("URL: ", Style::default().fg(Color::Cyan)),
            Span::styled(info.url.clone(), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Loading agent card...",
            Style::default().fg(Color::Yellow).italic(),
        )]),
    ]
}

fn build_no_agent_content(state: &AppState) -> Vec<Line<'static>> {
    if state.agents.is_loading {
        return vec![
            Line::from(vec![Span::styled(
                "Discovering agents...",
                Style::default().fg(Color::Yellow).italic(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Connecting to registry",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
    }

    if let Some(ref error) = state.agents.error {
        return vec![
            Line::from(vec![Span::styled(
                "Discovery failed",
                Style::default().fg(Color::Red).bold(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                error.clone(),
                Style::default().fg(Color::Red).italic(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press 'r' to retry",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
    }

    if state.agents.available_agents.is_empty() {
        return vec![
            Line::from(vec![Span::styled(
                "No agents available",
                Style::default().fg(Color::DarkGray).italic(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press 'r' in Agents tab",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "to refresh",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
    }

    vec![
        Line::from(vec![Span::styled(
            "No agent selected",
            Style::default().fg(Color::DarkGray).italic(),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Go to Agents tab (2)",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(vec![Span::styled(
            "to select an agent",
            Style::default().fg(Color::DarkGray),
        )]),
    ]
}
