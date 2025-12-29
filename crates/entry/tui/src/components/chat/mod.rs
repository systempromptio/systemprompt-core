mod execution_timeline;
mod input_request;
mod message_formatting;
mod message_list;
mod task_detail;
mod tool_panel;

pub use execution_timeline::{render_execution_timeline, timeline_height};
pub use input_request::{render_input_request, should_show_input_request};
pub use message_list::render_chat_messages;
pub use task_detail::{render_task_detail, should_show_task_detail};
pub use tool_panel::{render_tool_panel, should_show_tool_panel};

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use super::border_color;
use crate::config::TuiConfig;
use crate::state::{AppState, FocusedPanel};

pub fn render_chat(frame: &mut Frame, area: Rect, state: &AppState, config: &TuiConfig) {
    let is_focused = state.focus == FocusedPanel::Chat;

    let context_subtitle = state
        .chat
        .context_id
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();

    let messages_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(config, is_focused)))
        .title(
            Line::from(vec![
                Span::styled("Context: ", Style::default().fg(Color::DarkGray)),
                Span::styled(context_subtitle, Style::default().fg(Color::Cyan)),
            ])
            .alignment(Alignment::Right),
        );

    let messages_inner = messages_block.inner(area);
    frame.render_widget(messages_block, area);

    render_chat_messages(frame, messages_inner, state, config);
}
