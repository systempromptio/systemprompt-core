use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::cli_registry::ExecutionMode;
use crate::messages::{Command, LogLevel, Message, ScrollDirection};
use crate::state::AppState;

pub fn handle_analytics_keys(key: KeyEvent, _state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => (
            Some(Message::AnalyticsScroll(ScrollDirection::Down)),
            Command::None,
        ),
        KeyCode::Up | KeyCode::Char('k') => (
            Some(Message::AnalyticsScroll(ScrollDirection::Up)),
            Command::None,
        ),
        KeyCode::Right | KeyCode::Char('l' | '1' | '2' | '3') => {
            (Some(Message::AnalyticsNextView), Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') => (Some(Message::AnalyticsPrevView), Command::None),
        KeyCode::Char('r') => (Some(Message::AnalyticsRefresh), Command::None),
        _ => (None, Command::None),
    }
}

pub fn handle_services_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.services.select_next_visible();
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.services.select_prev_visible();
            (None, Command::None)
        },
        KeyCode::Right | KeyCode::Char('l') => {
            state.services.toggle_selected_group();
            (None, Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') => {
            state.services.collapse_selected_group();
            (None, Command::None)
        },
        KeyCode::Char('r') => state.services.selected_service().map_or_else(
            || (None, Command::None),
            |service| (None, Command::RestartService(service.name.clone())),
        ),
        KeyCode::Char('s') => state.services.selected_service().map_or_else(
            || (None, Command::None),
            |service| (None, Command::StartService(service.name.clone())),
        ),
        KeyCode::Char('x') => state.services.selected_service().map_or_else(
            || (None, Command::None),
            |service| (None, Command::StopService(service.name.clone())),
        ),
        KeyCode::Enter => (Some(Message::ServiceRefresh), Command::RefreshServices),
        _ => (None, Command::None),
    }
}

pub fn handle_commands_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    if state.commands.is_modal_open() {
        return handle_commands_modal_keys(key, state);
    }

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.commands.select_next();
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.commands.select_previous();
            (None, Command::None)
        },
        KeyCode::Right | KeyCode::Char('l') => {
            state.commands.expand_current();
            (None, Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') => {
            state.commands.collapse_current();
            (None, Command::None)
        },
        KeyCode::Tab => {
            state.commands.toggle_expanded();
            (None, Command::None)
        },
        KeyCode::Enter => handle_commands_execute(state),
        KeyCode::PageUp => {
            state.commands.scroll_output_up(10);
            (None, Command::None)
        },
        KeyCode::PageDown => {
            state.commands.scroll_output_down(10);
            (None, Command::None)
        },
        _ => (None, Command::None),
    }
}

fn handle_commands_execute(state: &mut AppState) -> (Option<Message>, Command) {
    let Some(item) = state.commands.selected_item().cloned() else {
        return (None, Command::None);
    };

    match item {
        crate::cli_registry::CommandTreeItem::Domain { .. } => {
            state.commands.toggle_expanded();
            (None, Command::None)
        },
        crate::cli_registry::CommandTreeItem::Command { info, .. } => match info.execution_mode {
            ExecutionMode::AiAssisted => {
                let command_path: Vec<String> = info.path.iter().map(ToString::to_string).collect();
                let description = info.description.to_string();
                (
                    Some(Message::CommandRequestAiParams {
                        command_path,
                        description,
                    }),
                    Command::None,
                )
            },
            ExecutionMode::Deterministic => {
                if info.arguments.is_empty() {
                    let cmd_string = format!("systemprompt {}", info.path.join(" "));
                    (
                        Some(Message::CommandExecuting),
                        Command::ExecuteCli(cmd_string),
                    )
                } else {
                    state.commands.open_parameter_modal();
                    (Some(Message::CommandModalOpen), Command::None)
                }
            },
        },
    }
}

fn handle_commands_modal_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    let Some(modal) = &mut state.commands.modal_state else {
        return (None, Command::None);
    };

    match key.code {
        KeyCode::Esc => {
            state.commands.close_modal();
            (Some(Message::CommandModalClose), Command::None)
        },
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                modal.prev_field();
            } else {
                modal.next_field();
            }
            (None, Command::None)
        },
        KeyCode::Down => {
            modal.next_field();
            (None, Command::None)
        },
        KeyCode::Up => {
            modal.prev_field();
            (None, Command::None)
        },
        KeyCode::Left => {
            modal.move_cursor_left();
            (None, Command::None)
        },
        KeyCode::Right => {
            modal.move_cursor_right();
            (None, Command::None)
        },
        KeyCode::Char(c) => {
            modal.insert_char(c);
            (None, Command::None)
        },
        KeyCode::Backspace => {
            modal.delete_char();
            (None, Command::None)
        },
        KeyCode::Enter => {
            if modal.validate() {
                let cmd_string = modal.build_command_string();
                state.commands.close_modal();
                (
                    Some(Message::CommandExecuting),
                    Command::ExecuteCli(cmd_string),
                )
            } else {
                (None, Command::None)
            }
        },
        _ => (None, Command::None),
    }
}

pub fn handle_logs_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.logs.scroll_down(1);
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.logs.scroll_up(1);
            (None, Command::None)
        },
        KeyCode::PageDown => {
            state.logs.scroll_down(10);
            (None, Command::None)
        },
        KeyCode::PageUp => {
            state.logs.scroll_up(10);
            (None, Command::None)
        },
        KeyCode::Char('G') => {
            state.logs.scroll_to_bottom();
            (None, Command::None)
        },
        KeyCode::Char('g') => {
            state.logs.scroll_offset = 0;
            (None, Command::None)
        },
        KeyCode::Char('e') => {
            state.logs.set_level_filter(Some(LogLevel::Error));
            (
                Some(Message::LogsSetFilter(Some(LogLevel::Error))),
                Command::None,
            )
        },
        KeyCode::Char('w') => {
            state.logs.set_level_filter(Some(LogLevel::Warn));
            (
                Some(Message::LogsSetFilter(Some(LogLevel::Warn))),
                Command::None,
            )
        },
        KeyCode::Char('i') => {
            state.logs.set_level_filter(Some(LogLevel::Info));
            (
                Some(Message::LogsSetFilter(Some(LogLevel::Info))),
                Command::None,
            )
        },
        KeyCode::Char('d') => {
            state.logs.set_level_filter(Some(LogLevel::Debug));
            (
                Some(Message::LogsSetFilter(Some(LogLevel::Debug))),
                Command::None,
            )
        },
        KeyCode::Char('a') => {
            state.logs.set_level_filter(None);
            (Some(Message::LogsSetFilter(None)), Command::None)
        },
        KeyCode::Char('f') => {
            state.logs.toggle_follow();
            (Some(Message::LogsToggleFollow), Command::None)
        },
        KeyCode::Char('c') => {
            state.logs.clear();
            (Some(Message::LogsClear), Command::None)
        },
        KeyCode::Char('r') => (Some(Message::LogsRefresh), Command::None),
        _ => (None, Command::None),
    }
}

pub fn handle_users_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.users.select_next();
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.users.select_prev();
            (None, Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') => {
            state.users.select_prev_role();
            (None, Command::None)
        },
        KeyCode::Right | KeyCode::Char('l') => {
            state.users.select_next_role();
            (None, Command::None)
        },
        KeyCode::Char(' ') => {
            if let Some((user_id, role, should_add)) = state.users.toggle_selected_role() {
                let role_value = if should_add {
                    format!("+{}", role)
                } else {
                    format!("-{}", role)
                };
                (
                    None,
                    Command::UpdateUserRole {
                        user_id,
                        role: role_value,
                    },
                )
            } else {
                (None, Command::None)
            }
        },
        KeyCode::Char('r') => (Some(Message::UsersRefresh), Command::RefreshUsers),
        _ => (None, Command::None),
    }
}

pub fn handle_conversations_keys(
    key: KeyEvent,
    state: &mut AppState,
) -> (Option<Message>, Command) {
    if state.conversations.editing {
        return handle_conversations_edit_keys(key, state);
    }

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.conversations.select_next();
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.conversations.select_prev();
            (None, Command::None)
        },
        KeyCode::Enter => state.conversations.selected_context_id().map_or_else(
            || (None, Command::None),
            |context_id| {
                (
                    Some(Message::ConversationSelect(context_id.to_string())),
                    Command::SelectConversation(context_id),
                )
            },
        ),
        KeyCode::Char('e') => {
            state.conversations.start_edit();
            (None, Command::None)
        },
        KeyCode::Char('d') => state.conversations.delete_selected().map_or_else(
            || (None, Command::None),
            |context_id| {
                (
                    Some(Message::ConversationDeleted(context_id.to_string())),
                    Command::DeleteConversation(context_id.to_string()),
                )
            },
        ),
        KeyCode::Char('n') => {
            let name = format!(
                "Conversation {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M")
            );
            (None, Command::CreateConversation(name))
        },
        KeyCode::Char('r') => (
            Some(Message::ConversationsRefresh),
            Command::RefreshConversations,
        ),
        _ => (None, Command::None),
    }
}

fn handle_conversations_edit_keys(
    key: KeyEvent,
    state: &mut AppState,
) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Esc => {
            state.conversations.cancel_edit();
            (None, Command::None)
        },
        KeyCode::Enter => {
            if let Some((context_id, new_name)) = state.conversations.finish_edit() {
                (
                    None,
                    Command::RenameConversation {
                        context_id,
                        name: new_name,
                    },
                )
            } else {
                (None, Command::None)
            }
        },
        KeyCode::Char(c) => {
            state.conversations.edit_push_char(c);
            (None, Command::None)
        },
        KeyCode::Backspace => {
            state.conversations.edit_pop_char();
            (None, Command::None)
        },
        _ => (None, Command::None),
    }
}
