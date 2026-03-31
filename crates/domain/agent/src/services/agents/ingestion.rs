use crate::models::Agent;
use crate::repository::content::AgentRepository;
use anyhow::{Result, anyhow};
use std::collections::HashSet;
use std::path::Path;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentId, SourceId};
use systemprompt_models::{
    AGENT_CONFIG_FILENAME, DiskAgentConfig, IngestionReport, strip_frontmatter,
};

#[derive(Debug)]
pub struct AgentIngestionService {
    agent_repo: AgentRepository,
}

impl AgentIngestionService {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            agent_repo: AgentRepository::new(db)?,
        })
    }

    pub async fn ingest_directory(
        &self,
        path: &Path,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<IngestionReport> {
        let mut report = IngestionReport::new();

        let agent_dirs = self.scan_agent_directories(path)?;
        report.files_found = agent_dirs.len();

        for agent_dir in agent_dirs {
            match self
                .ingest_agent(&agent_dir, source_id.clone(), override_existing)
                .await
            {
                Ok(()) => {
                    report.files_processed += 1;
                },
                Err(e) => {
                    report
                        .errors
                        .push(format!("{}: {}", agent_dir.display(), e));
                },
            }
        }

        Ok(report)
    }

    async fn ingest_agent(
        &self,
        agent_dir: &Path,
        source_id: SourceId,
        override_existing: bool,
    ) -> Result<()> {
        let config_path = agent_dir.join(AGENT_CONFIG_FILENAME);

        if !config_path.exists() {
            return Err(anyhow!(
                "No {} found in agent directory",
                AGENT_CONFIG_FILENAME
            ));
        }

        let dir_name = agent_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("Invalid agent directory name"))?;

        let config_text = std::fs::read_to_string(&config_path)?;
        let config: DiskAgentConfig = serde_yaml::from_str(&config_text)
            .map_err(|e| anyhow!("Failed to parse {}: {}", AGENT_CONFIG_FILENAME, e))?;

        let agent_id_str = if config.id.is_empty() {
            dir_name.replace('-', "_")
        } else {
            config.id.clone()
        };

        let system_prompt_path = agent_dir.join(config.system_prompt_file());
        let system_prompt = if system_prompt_path.exists() {
            let raw = std::fs::read_to_string(&system_prompt_path)?;
            Some(strip_frontmatter(&raw))
        } else {
            None
        };

        let endpoint = config
            .endpoint
            .clone()
            .unwrap_or_else(|| format!("/api/v1/agents/{}", config.name));

        let card_json = serde_json::to_value(&config.card)
            .map_err(|e| anyhow!("Failed to serialize agent card: {}", e))?;

        let agent = Agent {
            agent_id: AgentId::new(&agent_id_str),
            name: config.name,
            display_name: config.display_name,
            description: config.description,
            version: config.version,
            system_prompt,
            enabled: config.enabled,
            port: i32::from(config.port),
            endpoint,
            dev_only: config.dev_only,
            is_primary: config.is_primary,
            is_default: config.default,
            tags: config.tags,
            category_id: None,
            source_id,
            provider: config.provider,
            model: config.model,
            mcp_servers: config.mcp_servers,
            skills: config.skills,
            card_json,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        if self
            .agent_repo
            .get_by_agent_id(&agent.agent_id)
            .await?
            .is_some()
        {
            if override_existing {
                self.agent_repo.update(&agent.agent_id, &agent).await?;
            }
        } else {
            self.agent_repo.create(&agent).await?;
        }

        Ok(())
    }

    fn scan_agent_directories(&self, dir: &Path) -> Result<Vec<std::path::PathBuf>> {
        use walkdir::WalkDir;

        let mut agent_dirs = Vec::new();
        let mut seen = HashSet::new();

        for entry in WalkDir::new(dir).max_depth(2).into_iter().filter_map(|e| {
            e.map_err(|err| {
                tracing::warn!(error = %err, "Skipping unreadable directory entry during agent scan");
                err
            })
            .ok()
        }) {
            if entry.file_type().is_dir() && entry.file_name() != "." {
                let config_file = entry.path().join(AGENT_CONFIG_FILENAME);
                if config_file.exists() {
                    let path = entry.path().to_path_buf();
                    if !seen.contains(&path) {
                        agent_dirs.push(path.clone());
                        seen.insert(path);
                    }
                }
            }
        }

        Ok(agent_dirs)
    }
}
