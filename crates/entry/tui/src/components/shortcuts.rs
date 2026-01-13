use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};

use super::{render_empty_state, table_header, table_highlight_style, TABLE_HIGHLIGHT_SYMBOL};
use crate::cli_registry::{CommandTreeItem, ExecutionMode};
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_shortcuts(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    render_command_tree(frame, chunks[0], state, config);
    render_output_panel(frame, chunks[1], state, config);
}

fn render_command_tree(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = Block::default()
        .title(" CLI Commands ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_unfocused));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.commands.visible_items.is_empty() {
        render_empty_state(frame, inner, "No commands available");
        return;
    }

    let header = table_header(&["", "Command", "Description"]);

    let rows = state.commands.visible_items.iter().map(|item| {
        let (icon, name, desc) = match item {
            CommandTreeItem::Domain {
                name,
                is_expanded,
                child_count,
                depth,
                ..
            } => {
                let indent = "  ".repeat(*depth);
                let expand_icon = if *is_expanded { "▼" } else { "▶" };
                (
                    format!("{indent}{expand_icon}"),
                    name.to_string(),
                    format!("({child_count} commands)"),
                )
            },
            CommandTreeItem::Command { info, depth } => {
                let indent = "  ".repeat(*depth);
                let mode_icon = match info.execution_mode {
                    ExecutionMode::AiAssisted => "◆",
                    ExecutionMode::Deterministic => "○",
                };
                (
                    format!("{indent}{mode_icon}"),
                    info.name.to_string(),
                    info.description.to_string(),
                )
            },
        };

        Row::new(vec![
            Cell::from(icon).style(Style::default().fg(Color::DarkGray)),
            Cell::from(name).style(Style::default().fg(Color::Yellow)),
            Cell::from(desc),
        ])
    });

    let widths = [
        Constraint::Length(8),
        Constraint::Length(20),
        Constraint::Min(30),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(table_highlight_style())
        .highlight_symbol(TABLE_HIGHLIGHT_SYMBOL);

    let mut table_state = TableState::default();
    table_state.select(Some(state.commands.selected_index));

    frame.render_stateful_widget(table, inner, &mut table_state);
}

fn render_output_panel(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let title = if state.commands.is_executing {
        " Output (executing...) "
    } else {
        " Output "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_unfocused));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let output_text = state
        .commands
        .output
        .as_deref()
        .unwrap_or("Select a command and press Enter to execute\n\n○ Deterministic - opens parameter form\n◆ AI-assisted - routes to chat for help");

    let paragraph = Paragraph::new(output_text)
        .wrap(Wrap { trim: false })
        .scroll((state.commands.output_scroll as u16, 0));

    frame.render_widget(paragraph, inner);
}
