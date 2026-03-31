use super::{compute_agent_hash, compute_db_agent_hash};
use crate::models::{AgentDiffItem, AgentsDiffResult, DiffStatus, DiskAgent};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::Path;
use systemprompt_agent::models::Agent;
use systemprompt_agent::repository::content::AgentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AgentId;
use systemprompt_models::{AGENT_CONFIG_FILENAME, DiskAgentConfig, strip_frontmatter};
use tracing::warn;

#[derive(Debug)]
pub struct AgentsDiffCalculator {
    agent_repo: AgentRepository,
}

impl AgentsDiffCalculator {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            agent_repo: AgentRepository::new(db)?,
        })
    }

    pub async fn calculate_diff(&self, agents_path: &Path) -> Result<AgentsDiffResult> {
        let db_agents = self.agent_repo.list_all().await?;
        let db_map: HashMap<AgentId, Agent> = db_agents
            .into_iter()
            .map(|a| (a.id.clone(), a))
            .collect();

        let disk_agents = Self::scan_disk_agents(agents_path)?;

        let mut result = AgentsDiffResult::default();

        for (agent_id, disk_agent) in &disk_agents {
            let disk_hash = compute_agent_hash(disk_agent);

            match db_map.get(agent_id) {
                None => {
                    result.added.push(AgentDiffItem {
                        agent_id: agent_id.clone(),
                        name: disk_agent.name.clone(),
                        status: DiffStatus::Added,
                        disk_hash: Some(disk_hash),
                        db_hash: None,
                    });
                },
                Some(db_agent) => {
                    let db_hash = compute_db_agent_hash(db_agent);
                    if db_hash == disk_hash {
                        result.unchanged += 1;
                    } else {
                        result.modified.push(AgentDiffItem {
                            agent_id: agent_id.clone(),
                            name: disk_agent.name.clone(),
                            status: DiffStatus::Modified,
                            disk_hash: Some(disk_hash),
                            db_hash: Some(db_hash),
                        });
                    }
                },
            }
        }

        for (agent_id, db_agent) in &db_map {
            if !disk_agents.contains_key(agent_id) {
                result.removed.push(AgentDiffItem {
                    agent_id: agent_id.clone(),
                    name: db_agent.name.clone(),
                    status: DiffStatus::Removed,
                    disk_hash: None,
                    db_hash: Some(compute_db_agent_hash(db_agent)),
                });
            }
        }

        Ok(result)
    }

    fn scan_disk_agents(path: &Path) -> Result<HashMap<AgentId, DiskAgent>> {
        let mut agents = HashMap::new();

        if !path.exists() {
            return Ok(agents);
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let agent_path = entry.path();

            if !agent_path.is_dir() {
                continue;
            }

            let config_path = agent_path.join(AGENT_CONFIG_FILENAME);
            if !config_path.exists() {
                continue;
            }

            match parse_agent_dir(&config_path, &agent_path) {
                Ok(agent) => {
                    agents.insert(agent.agent_id.clone(), agent);
                },
                Err(e) => {
                    warn!("Failed to parse agent at {}: {}", agent_path.display(), e);
                },
            }
        }

        Ok(agents)
    }
}

fn parse_agent_dir(config_path: &Path, agent_dir: &Path) -> Result<DiskAgent> {
    let config_text = std::fs::read_to_string(config_path)?;
    let config: DiskAgentConfig = serde_yaml::from_str(&config_text)?;

    let dir_name = agent_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid agent directory name"))?;

    let agent_id_str = if config.id.is_empty() {
        dir_name.replace('-', "_")
    } else {
        config.id.clone()
    };
    let agent_id = AgentId::new(agent_id_str);

    let system_prompt_path = agent_dir.join(config.system_prompt_file());
    let system_prompt = if system_prompt_path.exists() {
        let raw = std::fs::read_to_string(&system_prompt_path)?;
        Some(strip_frontmatter(&raw))
    } else {
        None
    };

    Ok(DiskAgent {
        agent_id,
        name: config.name,
        display_name: config.display_name,
        description: config.description,
        system_prompt,
        port: config.port,
    })
}
