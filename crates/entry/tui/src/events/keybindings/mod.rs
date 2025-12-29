mod dialogs;
mod tabs;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::TuiConfig;
use crate::messages::{Command, Message};
use crate::state::{ActiveTab, AppState};

pub fn handle_key_event(
    key: KeyEvent,
    state: &mut AppState,
    _config: &TuiConfig,
) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Tab if key.modifiers.is_empty() => {
            state.next_tab();
            return (Some(Message::SwitchTab(state.active_tab)), Command::None);
        },
        KeyCode::BackTab => {
            state.prev_tab();
            return (Some(Message::SwitchTab(state.active_tab)), Command::None);
        },
        _ => {},
    }

    if state.chat.has_task_selected() && state.chat.input_buffer.is_empty() {
        return dialogs::handle_task_detail_keys(key, state);
    }

    if state.chat.selected_tool_index.is_some() {
        return dialogs::handle_tool_panel_keys(key, state);
    }

    if state.chat.has_pending_input() {
        return dialogs::handle_input_request_keys(key, state);
    }

    if state.has_pending_approval() {
        return dialogs::handle_approval_keys(key, state);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if state.chat.is_processing() {
            return (Some(Message::ChatCancelStream), Command::CancelAiStream);
        }
        return (Some(Message::Quit), Command::Quit);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
        return (Some(Message::ChatClearConversation), Command::None);
    }

    if matches!(state.active_tab, ActiveTab::Chat) {
        if let Some(result) = tabs::handle_chat_input(key, state) {
            return result;
        }
    }

    match state.active_tab {
        ActiveTab::Chat => tabs::handle_chat_keys(key, state),
        ActiveTab::Conversations => tabs::handle_conversations_keys(key, state),
        ActiveTab::Agents => tabs::handle_agents_keys(key, state),
        ActiveTab::Artifacts => tabs::handle_artifacts_keys(key, state),
        ActiveTab::Services => tabs::handle_services_keys(key, state),
        ActiveTab::Shortcuts => tabs::handle_commands_keys(key, state),
        ActiveTab::Logs => tabs::handle_logs_keys(key, state),
        ActiveTab::Users => tabs::handle_users_keys(key, state),
        ActiveTab::Analytics => tabs::handle_analytics_keys(key, state),
        ActiveTab::Config => (None, Command::None),
    }
}
