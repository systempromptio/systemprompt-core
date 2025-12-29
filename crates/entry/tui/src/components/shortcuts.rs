use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap};

use super::{render_empty_state, table_header, table_highlight_style, TABLE_HIGHLIGHT_SYMBOL};
use crate::config::TuiConfig;
use crate::state::AppState;

pub fn render_shortcuts(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let chunks =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

    render_command_list(frame, chunks[0], state, config);
    render_output_panel(frame, chunks[1], state, config);
}

fn render_command_list(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let block = Block::default()
        .title(" Commands ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(config.theme.border_unfocused));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.commands.commands.is_empty() {
        render_empty_state(frame, inner, "No commands available");
        return;
    }

    let header = table_header(&["Command", "Description"]);

    let rows = state.commands.commands.iter().map(|cmd| {
        Row::new(vec![
            Cell::from(cmd.slash_command.clone()).style(Style::default().fg(Color::Yellow)),
            Cell::from(cmd.description.clone()),
        ])
    });

    let widths = [Constraint::Length(25), Constraint::Min(30)];

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
        .unwrap_or("Select a command and press Enter to execute");

    let paragraph = Paragraph::new(output_text)
        .wrap(Wrap { trim: false })
        .scroll((state.commands.output_scroll as u16, 0));

    frame.render_widget(paragraph, inner);
}
