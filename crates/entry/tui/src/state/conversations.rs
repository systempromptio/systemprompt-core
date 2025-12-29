use std::time::Instant;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::ContextId;

#[derive(Debug, Clone)]
pub struct ConversationDisplay {
    pub context_id: ContextId,
    pub name: String,
    pub task_count: i64,
    pub message_count: i64,
    pub last_message_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct ConversationsState {
    pub conversations: Vec<ConversationDisplay>,
    pub selected_index: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub last_refresh: Option<Instant>,
}

impl ConversationsState {
    pub const fn new() -> Self {
        Self {
            conversations: Vec::new(),
            selected_index: 0,
            editing: false,
            edit_buffer: String::new(),
            last_refresh: None,
        }
    }

    pub fn update_conversations(&mut self, conversations: Vec<ConversationDisplay>) {
        self.conversations = conversations;
        self.last_refresh = Some(Instant::now());
        if self.selected_index >= self.conversations.len() {
            self.selected_index = self.conversations.len().saturating_sub(1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.conversations.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.conversations.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.conversations.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.conversations.len() - 1);
        }
    }

    pub fn selected_conversation(&self) -> Option<&ConversationDisplay> {
        self.conversations.get(self.selected_index)
    }

    pub fn selected_context_id(&self) -> Option<ContextId> {
        self.selected_conversation().map(|c| c.context_id.clone())
    }

    pub fn start_edit(&mut self) {
        if let Some(conv) = self.selected_conversation() {
            self.edit_buffer = conv.name.clone();
            self.editing = true;
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    pub fn finish_edit(&mut self) -> Option<(ContextId, String)> {
        if !self.editing {
            return None;
        }

        let new_name = self.edit_buffer.trim().to_string();
        if new_name.is_empty() {
            self.cancel_edit();
            return None;
        }

        let context_id = self.selected_context_id()?;

        if let Some(conv) = self.conversations.get_mut(self.selected_index) {
            conv.name.clone_from(&new_name);
        }

        self.editing = false;
        self.edit_buffer.clear();

        Some((context_id, new_name))
    }

    pub fn delete_selected(&mut self) -> Option<ContextId> {
        if self.conversations.is_empty() {
            return None;
        }

        let context_id = self.selected_context_id()?;
        self.conversations.remove(self.selected_index);

        if self.selected_index >= self.conversations.len() && !self.conversations.is_empty() {
            self.selected_index = self.conversations.len() - 1;
        }

        Some(context_id)
    }

    pub fn edit_push_char(&mut self, c: char) {
        if self.editing {
            self.edit_buffer.push(c);
        }
    }

    pub fn edit_pop_char(&mut self) {
        if self.editing {
            self.edit_buffer.pop();
        }
    }
}

impl Default for ConversationsState {
    fn default() -> Self {
        Self::new()
    }
}
