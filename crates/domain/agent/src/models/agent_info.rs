use crate::models::a2a::{AgentCard, AgentSkill};
use serde::{Deserialize, Serialize};
use systemprompt_models::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub card: AgentCard,
    pub enabled: bool,
    pub skills: Option<Vec<AgentSkill>>,
    pub mcp_servers: Option<Vec<String>>,
}

impl AgentInfo {
    pub const fn from_repository_data(agent_id: String, card: AgentCard, enabled: bool) -> Self {
        Self {
            agent_id,
            card,
            enabled,
            skills: None,
            mcp_servers: None,
        }
    }

    pub const fn from_card(agent_id: String, card: AgentCard, enabled: bool) -> Self {
        Self {
            agent_id,
            card,
            enabled,
            skills: None,
            mcp_servers: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.agent_id
    }

    pub fn name(&self) -> &str {
        &self.card.name
    }

    pub fn endpoint(&self) -> &str {
        &self.card.url
    }

    pub fn full_endpoint(&self) -> String {
        let endpoint = &self.card.url;
        if endpoint.starts_with('/') {
            let base_url = Config::get()
                .map(|c| c.api_external_url.clone())
                .unwrap_or_else(|_| "http://localhost:3000".to_string());
            format!("{}{}", base_url, endpoint)
        } else {
            endpoint.to_string()
        }
    }

    pub fn version(&self) -> &str {
        &self.card.version
    }

    pub fn with_skills(mut self, skills: Vec<AgentSkill>) -> Self {
        self.skills = Some(skills);
        self
    }

    pub fn with_mcp_servers(mut self, servers: Vec<String>) -> Self {
        self.mcp_servers = Some(servers);
        self
    }

    pub fn skills_count(&self) -> usize {
        self.skills.as_ref().map(|s| s.len()).unwrap_or(0)
    }

    pub fn mcp_count(&self) -> usize {
        self.mcp_servers.as_ref().map(|s| s.len()).unwrap_or(0)
    }
}
