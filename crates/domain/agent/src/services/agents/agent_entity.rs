use crate::models::Agent;
use crate::repository::content::AgentRepository;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AgentId;

#[derive(Debug)]
pub struct AgentEntityService {
    agent_repo: Arc<AgentRepository>,
}

impl AgentEntityService {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            agent_repo: Arc::new(AgentRepository::new(db)?),
        })
    }

    pub async fn get_agent(&self, agent_id: &AgentId) -> Result<Option<Agent>> {
        self.agent_repo.get_by_agent_id(agent_id).await
    }

    pub async fn get_agent_by_name(&self, name: &str) -> Result<Option<Agent>> {
        self.agent_repo.get_by_name(name).await
    }

    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        self.agent_repo.list_all().await
    }

    pub async fn list_enabled_agents(&self) -> Result<Vec<Agent>> {
        self.agent_repo.list_enabled().await
    }

    pub async fn create_agent(&self, agent: &Agent) -> Result<()> {
        self.agent_repo.create(agent).await
    }

    pub async fn update_agent(&self, agent_id: &AgentId, agent: &Agent) -> Result<()> {
        self.agent_repo.update(agent_id, agent).await
    }

    pub async fn delete_agent(&self, agent_id: &AgentId) -> Result<()> {
        self.agent_repo.delete(agent_id).await
    }
}
