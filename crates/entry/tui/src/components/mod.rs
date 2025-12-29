mod agent_card;
mod agents;
mod analytics;
mod approval;
pub mod artifact_renderers;
mod artifacts;
pub mod chat;
mod config;
mod control_guide;
mod conversations;
mod global_input;
mod logs_tab;
mod shortcuts;
mod sidebar;
mod spinner;
mod tabs;
mod users;

pub use agent_card::render_agent_card;
pub use agents::render_agents;
pub use analytics::render_analytics;
pub use approval::render_approval_dialog;
pub use artifacts::{render_artifacts, ArtifactContext};
pub use config::render_config;
pub use control_guide::{control_guide_width, render_control_guide};
pub use conversations::render_conversations;
pub use global_input::render_global_input;
pub use logs_tab::render_logs_tab;
pub use shortcuts::render_shortcuts;
pub use sidebar::render_sidebar;
pub use spinner::get_spinner_frame;
pub use tabs::{build_tabs_line, render_tabs};
pub use users::render_users;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row};

pub use self::tab_content_block_with_tabs as tab_block_with_tabs;

use crate::config::TuiConfig;

pub const fn border_color(config: &TuiConfig, is_focused: bool) -> Color {
    if is_focused {
        config.theme.border_focused
    } else {
        config.theme.border_unfocused
    }
}

use crate::state::ActiveTab;

pub fn tab_content_block(config: &TuiConfig, is_focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(config, is_focused)))
}

pub fn tab_content_block_with_tabs(
    config: &TuiConfig,
    is_focused: bool,
    active_tab: ActiveTab,
) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(config, is_focused)))
        .title(build_tabs_line(active_tab))
        .title_alignment(Alignment::Left)
}

pub fn content_pane_block(config: &TuiConfig) -> Block<'static> {
    tab_content_block(config, false)
}

pub fn inner_panel_block<'a>(config: &TuiConfig, is_focused: bool, title: &'a str) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(config, is_focused)))
        .title(title)
}

pub fn inner_panel_block_untitled(config: &TuiConfig) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(config, false)))
}

pub const TABLE_HIGHLIGHT_SYMBOL: &str = "▸ ";

pub fn table_header<'a>(columns: &[&'a str]) -> Row<'a> {
    let cells = columns
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().bold().fg(Color::Cyan)));
    Row::new(cells).height(1).bottom_margin(1)
}

pub fn table_highlight_style() -> Style {
    Style::default()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

pub fn render_empty_state(frame: &mut Frame, area: Rect, message: &str) {
    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::DarkGray).italic())
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

pub fn render_loading_state(frame: &mut Frame, area: Rect, message: &str) {
    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

pub fn split_left_panel_block(config: &TuiConfig) -> Block<'static> {
    Block::default()
        .borders(Borders::LEFT | Borders::BOTTOM)
        .border_style(Style::default().fg(border_color(config, false)))
}

pub fn split_right_panel_block(config: &TuiConfig) -> Block<'static> {
    Block::default()
        .borders(Borders::RIGHT | Borders::BOTTOM)
        .border_style(Style::default().fg(border_color(config, false)))
}
