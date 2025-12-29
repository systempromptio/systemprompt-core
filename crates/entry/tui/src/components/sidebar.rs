use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem, ListState};

use super::tab_content_block;
use crate::config::TuiConfig;
use crate::state::{AppState, FocusedPanel, RuntimeStatus, ServiceListItem, ServiceType};

pub fn render_sidebar(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let is_focused = state.focus == FocusedPanel::Sidebar;

    let block = tab_content_block(config, is_focused);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_items = state.services.visible_items();
    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_index: Option<usize> = None;

    for (index, item) in visible_items.iter().enumerate() {
        if *item == state.services.selected_item {
            selected_index = Some(index);
        }

        match item {
            ServiceListItem::GroupHeader(group, _) => {
                items.push(create_group_header(*group, state, config));
            },
            ServiceListItem::Service(service_index) => {
                if let Some(service) = state.services.services.get(*service_index) {
                    items.push(create_service_item(service, config));
                }
            },
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "  No services configured",
            Style::default().fg(Color::DarkGray).italic(),
        )])));
    }

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(config.theme.brand_primary)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    list_state.select(selected_index);

    frame.render_stateful_widget(list, inner, &mut list_state);
}

fn create_group_header<'a>(
    group: ServiceType,
    state: &AppState,
    config: &TuiConfig,
) -> ListItem<'a> {
    let is_expanded = state.services.is_group_expanded(group);
    let arrow = if is_expanded { "▼" } else { "▶" };

    let (name, count) = match group {
        ServiceType::Api => {
            let count = state.services.api_services().count();
            ("API Server", count)
        },
        ServiceType::Agent => {
            let count = state.services.agent_services().count();
            ("Agents", count)
        },
        ServiceType::Mcp => {
            let count = state.services.mcp_services().count();
            ("MCP Servers", count)
        },
    };

    let running = state
        .services
        .services
        .iter()
        .filter(|service| service.service_type == group && service.status.is_healthy())
        .count();

    let status_indicator = if running == count && count > 0 {
        Span::styled(" ", Style::default().fg(config.theme.status_running))
    } else if running > 0 {
        Span::styled(" ", Style::default().fg(Color::Yellow))
    } else {
        Span::styled(" ", Style::default().fg(config.theme.status_stopped))
    };

    ListItem::new(Line::from(vec![
        Span::styled(format!("{} ", arrow), Style::default().fg(Color::Cyan)),
        Span::styled(name, Style::default().bold().fg(Color::White)),
        Span::styled(
            format!(" ({})", count),
            Style::default().fg(Color::DarkGray),
        ),
        status_indicator,
    ]))
}

fn create_service_item<'a>(
    service: &crate::state::ServiceStatus,
    config: &TuiConfig,
) -> ListItem<'a> {
    let status_color = match service.status {
        RuntimeStatus::Running => config.theme.status_running,
        RuntimeStatus::Stopped => config.theme.status_stopped,
        RuntimeStatus::Starting => Color::Yellow,
        RuntimeStatus::Crashed => config.theme.status_error,
        RuntimeStatus::Orphaned => Color::Magenta,
    };

    let port_text = service
        .port
        .map(|port| format!(":{port}"))
        .unwrap_or_default();

    ListItem::new(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            service.status_symbol().to_string(),
            Style::default().fg(status_color),
        ),
        Span::raw(" "),
        Span::styled(service.name.clone(), Style::default().fg(Color::White)),
        Span::styled(port_text, Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("  {}", service.status),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
}
