mod types;

pub use types::*;

use std::collections::HashMap;
use systemprompt_models::AgentCard;

#[derive(Debug)]
pub struct AgentsState {
    pub available_agents: Vec<AgentInfo>,
    pub active_agent: Option<String>,
    pub cursor_index: usize,
    pub agent_cards: HashMap<String, AgentCard>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub expanded_index: Option<usize>,
    pub display_metadata: HashMap<String, AgentDisplayMetadata>,
    pub instructions_expanded: bool,
}

impl Default for AgentsState {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentsState {
    pub fn new() -> Self {
        Self {
            available_agents: Vec::new(),
            active_agent: None,
            cursor_index: 0,
            agent_cards: HashMap::new(),
            is_loading: false,
            error: None,
            expanded_index: None,
            display_metadata: HashMap::new(),
            instructions_expanded: false,
        }
    }

    pub fn toggle_instructions_expanded(&mut self) {
        self.instructions_expanded = !self.instructions_expanded;
    }

    pub fn get_selected_display_metadata(&self) -> Option<&AgentDisplayMetadata> {
        self.active_agent
            .as_ref()
            .and_then(|name| self.display_metadata.get(name))
    }

    pub fn set_agent_display_metadata(&mut self, name: &str, metadata: AgentDisplayMetadata) {
        self.display_metadata.insert(name.to_string(), metadata);
    }

    pub fn set_agents(&mut self, agents: Vec<AgentInfo>) {
        self.available_agents = agents;

        if self.active_agent.is_none() && !self.available_agents.is_empty() {
            let primary = self
                .available_agents
                .iter()
                .find(|a| a.is_primary)
                .or_else(|| self.available_agents.first());

            if let Some(agent) = primary {
                self.active_agent = Some(agent.name.clone());
                self.cursor_index = self
                    .available_agents
                    .iter()
                    .position(|a| a.name == agent.name)
                    .unwrap_or(0);
            }
        }
    }

    pub fn set_agents_with_cards(&mut self, cards: Vec<AgentCard>) {
        self.agent_cards.clear();

        self.available_agents = cards.iter().map(AgentInfo::from_card).collect();

        for card in cards {
            self.agent_cards.insert(card.name.clone(), card);
        }

        if self.active_agent.is_none() && !self.available_agents.is_empty() {
            let primary = self
                .available_agents
                .iter()
                .find(|a| a.is_primary)
                .or_else(|| self.available_agents.first());

            if let Some(agent) = primary {
                self.active_agent = Some(agent.name.clone());
                self.cursor_index = self
                    .available_agents
                    .iter()
                    .position(|a| a.name == agent.name)
                    .unwrap_or(0);
            }
        }
    }

    pub fn select_agent(&mut self, name: &str) -> bool {
        if let Some(idx) = self.available_agents.iter().position(|a| a.name == name) {
            self.active_agent = Some(name.to_string());
            self.cursor_index = idx;
            true
        } else {
            false
        }
    }

    pub fn move_cursor_next(&mut self) {
        if !self.available_agents.is_empty() {
            self.cursor_index = (self.cursor_index + 1) % self.available_agents.len();
        }
    }

    pub fn move_cursor_prev(&mut self) {
        if !self.available_agents.is_empty() {
            self.cursor_index = if self.cursor_index == 0 {
                self.available_agents.len() - 1
            } else {
                self.cursor_index - 1
            };
        }
    }

    pub fn select_next(&mut self) {
        self.move_cursor_next();
        if let Some(agent) = self.available_agents.get(self.cursor_index) {
            self.active_agent = Some(agent.name.clone());
        }
    }

    pub fn select_previous(&mut self) {
        self.move_cursor_prev();
        if let Some(agent) = self.available_agents.get(self.cursor_index) {
            self.active_agent = Some(agent.name.clone());
        }
    }

    pub fn activate_current(&mut self) -> Option<&AgentInfo> {
        if let Some(agent) = self.available_agents.get(self.cursor_index) {
            self.active_agent = Some(agent.name.clone());
            Some(agent)
        } else {
            None
        }
    }

    pub fn get_cursor_agent(&self) -> Option<&AgentInfo> {
        self.available_agents.get(self.cursor_index)
    }

    pub fn get_selected_agent(&self) -> Option<&AgentInfo> {
        self.active_agent
            .as_ref()
            .and_then(|name| self.available_agents.iter().find(|a| &a.name == name))
    }

    pub fn get_selected_card(&self) -> Option<&AgentCard> {
        self.active_agent
            .as_ref()
            .and_then(|name| self.agent_cards.get(name))
    }

    pub fn get_cursor_card(&self) -> Option<&AgentCard> {
        self.available_agents
            .get(self.cursor_index)
            .and_then(|agent| self.agent_cards.get(&agent.name))
    }

    pub fn toggle_expanded(&mut self) {
        if self.expanded_index == Some(self.cursor_index) {
            self.expanded_index = None;
        } else {
            self.expanded_index = Some(self.cursor_index);
        }
    }

    pub fn collapse_expanded(&mut self) {
        self.expanded_index = None;
    }

    pub fn is_expanded(&self, index: usize) -> bool {
        self.expanded_index == Some(index)
    }

    pub fn is_active(&self, index: usize) -> bool {
        self.available_agents
            .get(index)
            .is_some_and(|agent| self.active_agent.as_ref() == Some(&agent.name))
    }

    pub fn set_agent_card(&mut self, name: &str, card: AgentCard) {
        self.agent_cards.insert(name.to_string(), card);
    }

    pub fn update_agent_status(&mut self, name: &str, status: AgentConnectionStatus) {
        if let Some(agent) = self.available_agents.iter_mut().find(|a| a.name == name) {
            agent.status = status;
        }
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }
}
