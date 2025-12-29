use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::messages::{Command, Message, ScrollDirection, SlashCommand};
use crate::state::AppState;

pub fn handle_chat_input(
    key: KeyEvent,
    state: &mut AppState,
) -> Option<(Option<Message>, Command)> {
    match key.code {
        KeyCode::Enter if key.modifiers.is_empty() => {
            if state.chat.is_processing() && !state.chat.current_inline_tools.is_empty() {
                if state.chat.selected_tool_index.is_none() {
                    state.chat.selected_tool_index = Some(0);
                }
                return Some((None, Command::None));
            }

            if !state.chat.input_buffer.trim().is_empty() && !state.chat.is_processing() {
                let input = state.chat.input_buffer.trim();

                if let Some(command) = SlashCommand::from_str(input) {
                    state.chat.clear_input();
                    return Some((Some(Message::SlashCommand(command)), Command::None));
                }

                return Some((Some(Message::ChatSend), Command::None));
            }
            Some((None, Command::None))
        },
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
            state.chat.input_buffer.push('\n');
            Some((None, Command::None))
        },
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.chat.input_buffer.push(c);
            Some((None, Command::None))
        },
        KeyCode::Backspace => {
            state.chat.input_buffer.pop();
            Some((None, Command::None))
        },
        _ => None,
    }
}

pub fn handle_chat_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    let input_empty = state.chat.input_buffer.is_empty();
    let not_processing = !state.chat.is_processing();

    match key.code {
        KeyCode::Char('i') if input_empty => {
            state.agents.toggle_instructions_expanded();
            (None, Command::None)
        },
        KeyCode::Char('t') if input_empty => {
            state.chat.toggle_execution_timeline();
            (None, Command::None)
        },
        KeyCode::Char('[') if state.chat.is_processing() => {
            state.chat.select_prev_tool();
            (None, Command::None)
        },
        KeyCode::Char(']') if state.chat.is_processing() => {
            state.chat.select_next_tool();
            (None, Command::None)
        },
        KeyCode::Up if input_empty && not_processing => {
            state.chat.select_prev_task();
            (Some(Message::ChatTaskSelectPrev), Command::None)
        },
        KeyCode::Down if input_empty && not_processing => {
            state.chat.select_next_task();
            (Some(Message::ChatTaskSelectNext), Command::None)
        },
        KeyCode::Enter
            if input_empty && not_processing && state.chat.selected_task_index.is_some() =>
        {
            (Some(Message::ChatTaskOpenDetail), Command::None)
        },
        KeyCode::Delete
            if input_empty && not_processing && state.chat.selected_task_index.is_some() =>
        {
            if let Some(task_id) = state.chat.selected_task_id() {
                state.chat.remove_task(&task_id);
                (Some(Message::ChatTaskDelete), Command::DeleteTask(task_id))
            } else {
                (None, Command::None)
            }
        },
        KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => (
            Some(Message::ChatScroll(ScrollDirection::Up)),
            Command::None,
        ),
        KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => (
            Some(Message::ChatScroll(ScrollDirection::Down)),
            Command::None,
        ),
        KeyCode::PageUp => (
            Some(Message::ChatScroll(ScrollDirection::PageUp)),
            Command::None,
        ),
        KeyCode::PageDown => (
            Some(Message::ChatScroll(ScrollDirection::PageDown)),
            Command::None,
        ),
        _ => (None, Command::None),
    }
}
