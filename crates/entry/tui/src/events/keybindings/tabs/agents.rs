use crossterm::event::{KeyCode, KeyEvent};

use crate::messages::{Command, Message};
use crate::state::AppState;

pub fn handle_agents_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.agents.move_cursor_next();
            (None, Command::None)
        },
        KeyCode::Up | KeyCode::Char('k') => {
            state.agents.move_cursor_prev();
            (None, Command::None)
        },
        KeyCode::Enter => state.agents.activate_current().map_or_else(
            || (None, Command::None),
            |agent| {
                let agent_name = agent.name.clone();
                (
                    Some(Message::AgentSelect(agent_name.clone())),
                    Command::AgentA2aSelect(agent_name),
                )
            },
        ),
        KeyCode::Right | KeyCode::Char('l') => {
            state.agents.toggle_expanded();
            (None, Command::None)
        },
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc => {
            state.agents.collapse_expanded();
            (None, Command::None)
        },
        KeyCode::Char('r') => (Some(Message::AgentsRefresh), Command::AgentsDiscover),
        _ => (None, Command::None),
    }
}

pub fn handle_artifacts_keys(key: KeyEvent, state: &mut AppState) -> (Option<Message>, Command) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => (Some(Message::ArtifactsSelectNext), Command::None),
        KeyCode::Up | KeyCode::Char('k') => (Some(Message::ArtifactsSelectPrevious), Command::None),
        KeyCode::Char('d') => state.artifacts.delete_selected().map_or_else(
            || (None, Command::None),
            |artifact_id| {
                (
                    Some(Message::ArtifactDeleted(artifact_id.clone())),
                    Command::DeleteArtifact(artifact_id.to_string()),
                )
            },
        ),
        KeyCode::Char('r') => (Some(Message::ArtifactsRefresh), Command::RefreshArtifacts),
        _ => (None, Command::None),
    }
}
