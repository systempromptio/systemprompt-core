use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};
use systemprompt_models::McpToolsParams;

use super::content_pane_block;
use crate::config::TuiConfig;
use crate::state::{AgentConnectionStatus, AppState};

pub fn render_agents(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = content_pane_block(config);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if render_loading_or_empty(frame, inner, state) {
        return;
    }

    let (table_area, detail_area) = calculate_layout(inner, state.agents.expanded_index.is_some());
    render_agents_table(frame, table_area, state);

    if let Some(detail_rect) = detail_area {
        render_agent_detail(frame, detail_rect, state, config);
    }
}

fn render_loading_or_empty(frame: &mut Frame, area: Rect, state: &AppState) -> bool {
    if state.agents.is_loading {
        let msg = Paragraph::new("Loading agents...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return true;
    }

    if state.agents.available_agents.is_empty() {
        let msg = Paragraph::new("No agents found. Press 'r' to refresh.")
            .style(Style::default().fg(Color::DarkGray).italic())
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return true;
    }

    false
}

fn calculate_layout(inner: Rect, has_detail: bool) -> (Rect, Option<Rect>) {
    if has_detail {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    }
}

fn render_agents_table(frame: &mut Frame, area: Rect, state: &AppState) {
    let header = build_table_header();
    let rows = state
        .agents
        .available_agents
        .iter()
        .enumerate()
        .map(|(idx, agent)| build_agent_row(agent, state.agents.is_active(idx)));

    let widths = [
        Constraint::Length(3),
        Constraint::Length(25),
        Constraint::Length(8),
        Constraint::Min(30),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut table_state = TableState::default();
    table_state.select(Some(state.agents.cursor_index));
    frame.render_stateful_widget(table, area, &mut table_state);
}

fn build_table_header() -> Row<'static> {
    let cells = ["", "Name", "Port", "URL"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().bold().fg(Color::Cyan)));
    Row::new(cells).height(1).bottom_margin(1)
}

fn build_agent_row(agent: &crate::state::AgentInfo, is_active: bool) -> Row<'static> {
    let (status_symbol, status_color) = agent_status_indicator(agent, is_active);
    let name_style = agent_name_style(agent, is_active);

    Row::new(vec![
        Cell::from(status_symbol).style(Style::default().fg(status_color)),
        Cell::from(agent.display_name.clone()).style(name_style),
        Cell::from(agent.port.to_string()),
        Cell::from(agent.url.clone()),
    ])
}

const fn agent_status_indicator(
    agent: &crate::state::AgentInfo,
    is_active: bool,
) -> (&'static str, Color) {
    if is_active {
        return ("●", Color::Cyan);
    }
    match &agent.status {
        AgentConnectionStatus::Connected => ("○", Color::Green),
        AgentConnectionStatus::Disconnected => ("○", Color::DarkGray),
        AgentConnectionStatus::Connecting => ("◐", Color::Yellow),
        AgentConnectionStatus::Error(_) => ("✗", Color::Red),
    }
}

fn agent_name_style(agent: &crate::state::AgentInfo, is_active: bool) -> Style {
    if is_active {
        Style::default().fg(Color::Cyan).bold()
    } else if agent.is_primary {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::White)
    }
}

fn render_agent_detail(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_unfocused))
        .title(Span::styled(
            " Agent Details ",
            Style::default().fg(Color::Cyan).bold(),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = build_detail_lines(state);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn build_detail_lines(state: &AppState) -> Vec<Line<'static>> {
    match (
        state.agents.get_cursor_agent(),
        state.agents.get_cursor_card(),
    ) {
        (Some(info), Some(card)) => build_full_agent_detail(info, card),
        (Some(info), None) => build_loading_detail(&info.display_name),
        _ => build_no_selection_detail(),
    }
}

fn build_full_agent_detail(
    info: &crate::state::AgentInfo,
    card: &systemprompt_models::AgentCard,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    append_header_line(&mut lines, &card.name, &info.status);
    append_description(&mut lines, &card.description);
    append_skills(&mut lines, &card.skills);
    append_mcp_servers(&mut lines, card.capabilities.extensions.as_ref());
    lines
}

fn append_header_line(lines: &mut Vec<Line<'static>>, name: &str, status: &AgentConnectionStatus) {
    let (status_text, status_style) = status_display(status);
    lines.push(Line::from(vec![
        Span::styled(name.to_string(), Style::default().fg(Color::White).bold()),
        Span::raw(" - "),
        Span::styled(status_text.to_string(), status_style),
    ]));
    lines.push(Line::from(""));
}

fn status_display(status: &AgentConnectionStatus) -> (&'static str, Style) {
    match status {
        AgentConnectionStatus::Connected => ("Connected", Style::default().fg(Color::Green)),
        AgentConnectionStatus::Connecting => ("Connecting...", Style::default().fg(Color::Yellow)),
        AgentConnectionStatus::Disconnected => {
            ("Disconnected", Style::default().fg(Color::DarkGray))
        },
        AgentConnectionStatus::Error(_) => ("Error", Style::default().fg(Color::Red)),
    }
}

fn append_description(lines: &mut Vec<Line<'static>>, description: &str) {
    if description.is_empty() {
        return;
    }
    lines.push(Line::from(Span::styled(
        "Description",
        Style::default().fg(Color::Cyan).bold(),
    )));
    for chunk in description.chars().collect::<Vec<_>>().chunks(80) {
        lines.push(Line::from(Span::styled(
            chunk.iter().collect::<String>(),
            Style::default().fg(Color::White),
        )));
    }
    lines.push(Line::from(""));
}

fn append_skills(lines: &mut Vec<Line<'static>>, skills: &[systemprompt_models::AgentSkill]) {
    if skills.is_empty() {
        return;
    }
    lines.push(Line::from(Span::styled(
        format!("Skills ({})", skills.len()),
        Style::default().fg(Color::Cyan).bold(),
    )));
    for skill in skills {
        lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(skill.name.clone(), Style::default().fg(Color::Yellow)),
        ]));
    }
    lines.push(Line::from(""));
}

fn append_mcp_servers(
    lines: &mut Vec<Line<'static>>,
    extensions: Option<&Vec<systemprompt_models::AgentExtension>>,
) {
    let Some(exts) = extensions else { return };
    let mcp_params = exts
        .iter()
        .find(|e| e.uri == "systemprompt:mcp-tools")
        .and_then(|e| e.params.as_ref())
        .and_then(|p| serde_json::from_value::<McpToolsParams>(p.clone()).ok());

    let Some(params) = mcp_params else { return };
    if params.servers.is_empty() {
        return;
    }

    lines.push(Line::from(Span::styled(
        format!("MCP Servers ({})", params.servers.len()),
        Style::default().fg(Color::Cyan).bold(),
    )));
    for server in &params.servers {
        let tool_count = server.tools.as_ref().map_or(0, Vec::len);
        let text = if tool_count > 0 {
            format!("{} ({} tools)", server.name, tool_count)
        } else {
            server.name.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(text, Style::default().fg(Color::Magenta)),
        ]));
    }
}

fn build_loading_detail(display_name: &str) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            display_name.to_string(),
            Style::default().fg(Color::White).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Loading agent card...",
            Style::default().fg(Color::Yellow).italic(),
        )),
    ]
}

fn build_no_selection_detail() -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        "No agent selected",
        Style::default().fg(Color::DarkGray).italic(),
    ))]
}
