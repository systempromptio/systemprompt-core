use crossterm::event::{KeyCode, KeyEvent};

use crate::messages::{Command, Message};
use crate::state::AppState;

pub fn handle_approval_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Char('y' | 'Y') => state.tools.approve_current().map_or_else(
            || (None, Command::None),
            |tool_call| {
                (
                    Some(Message::ToolApprove(tool_call.id)),
                    Command::ExecuteTool(tool_call.id),
                )
            },
        ),
        KeyCode::Char('n' | 'N') | KeyCode::Esc => state.tools.reject_current().map_or_else(
            || (None, Command::None),
            |id| (Some(Message::ToolReject(id)), Command::None),
        ),
        _ => (None, Command::None),
    }
}

pub fn handle_tool_panel_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.chat.close_tool_panel();
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.chat.scroll_tool_panel_up(1);
            (None, Command::None)
        },
        KeyCode::Down | KeyCode::Char('j') => {
            state.chat.scroll_tool_panel_down(1);
            (None, Command::None)
        },
        KeyCode::PageUp => {
            state.chat.scroll_tool_panel_up(10);
            (None, Command::None)
        },
        KeyCode::PageDown => {
            state.chat.scroll_tool_panel_down(10);
            (None, Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') => {
            state.chat.select_prev_tool();
            state.chat.tool_panel_scroll = 0;
            (None, Command::None)
        },
        KeyCode::Right | KeyCode::Char('l') => {
            state.chat.select_next_tool();
            state.chat.tool_panel_scroll = 0;
            (None, Command::None)
        },
        _ => (None, Command::None),
    }
}

pub fn handle_input_request_keys(
    key: KeyEvent,
    state: &mut AppState,
) -> (Option<Message>, Command) {
    let request_id = state
        .chat
        .pending_input()
        .map(|r| r.request_id.clone())
        .unwrap_or_default();

    let input_type = state
        .chat
        .pending_input()
        .map(|r| r.input_type)
        .unwrap_or_default();

    match key.code {
        KeyCode::Esc => {
            state.chat.clear_input_request();
            (None, Command::CancelInputRequest { request_id })
        },
        KeyCode::Enter => {
            let value = state.chat.get_input_value().unwrap_or_default();
            state.chat.clear_input_request();
            (None, Command::SendInputResponse { request_id, value })
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.chat.input_prev_choice();
            (None, Command::None)
        },
        KeyCode::Down | KeyCode::Char('j') => {
            state.chat.input_next_choice();
            (None, Command::None)
        },
        KeyCode::Char(c) => {
            if matches!(input_type, crate::state::InputType::Text) {
                state.chat.input_push_char(c);
            }
            (None, Command::None)
        },
        KeyCode::Backspace => {
            if matches!(input_type, crate::state::InputType::Text) {
                state.chat.input_pop_char();
            }
            (None, Command::None)
        },
        _ => (None, Command::None),
    }
}

pub fn handle_task_detail_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.chat.close_task_detail();
            (Some(Message::ChatTaskCloseDetail), Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.chat.scroll_task_detail_up(1);
            (None, Command::None)
        },
        KeyCode::Down | KeyCode::Char('j') => {
            state.chat.scroll_task_detail_down(1);
            (None, Command::None)
        },
        KeyCode::PageUp => {
            state.chat.scroll_task_detail_up(10);
            (None, Command::None)
        },
        KeyCode::PageDown => {
            state.chat.scroll_task_detail_down(10);
            (None, Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') => {
            state.chat.select_prev_task();
            state.chat.task_detail_scroll = 0;
            (Some(Message::ChatTaskSelectPrev), Command::None)
        },
        KeyCode::Right | KeyCode::Char('l') => {
            state.chat.select_next_task();
            state.chat.task_detail_scroll = 0;
            (Some(Message::ChatTaskSelectNext), Command::None)
        },
        KeyCode::Delete => {
            if let Some(task_id) = state.chat.selected_task_id() {
                state.chat.remove_task(&task_id);
                if state.chat.tasks.is_empty() {
                    state.chat.close_task_detail();
                }
                (Some(Message::ChatTaskDelete), Command::DeleteTask(task_id))
            } else {
                (None, Command::None)
            }
        },
        _ => (None, Command::None),
    }
}
