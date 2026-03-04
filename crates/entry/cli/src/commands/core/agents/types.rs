use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_models::{strip_frontmatter, DiskAgentConfig, AGENT_CONFIG_FILENAME};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentListOutput {
    pub agents: Vec<AgentSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSummary {
    pub agent_id: String,
    pub name: String,
    pub display_name: String,
    pub enabled: bool,
    pub port: u16,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentDetailOutput {
    pub agent_id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub port: u16,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
    pub system_prompt_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ListOrDetail {
    List(AgentListOutput),
    Detail(AgentDetailOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSyncOutput {
    pub direction: String,
    pub synced: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub struct ParsedAgent {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub enabled: bool,
    pub port: u16,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
    pub system_prompt: Option<String>,
}

pub fn parse_agent_from_config(config_path: &Path, agent_dir: &Path) -> Result<ParsedAgent> {
    let config_text = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let config: DiskAgentConfig = serde_yaml::from_str(&config_text)
        .with_context(|| format!("Invalid YAML in {}", config_path.display()))?;

    let system_prompt_path = agent_dir.join(config.system_prompt_file());
    let system_prompt = if system_prompt_path.exists() {
        let raw = std::fs::read_to_string(&system_prompt_path)
            .with_context(|| format!("Failed to read {}", system_prompt_path.display()))?;
        Some(strip_frontmatter(&raw))
    } else {
        None
    };

    Ok(ParsedAgent {
        name: config.name,
        display_name: config.display_name,
        description: config.description,
        enabled: config.enabled,
        port: config.port,
        tags: config.tags,
        category: config.category,
        mcp_servers: config.mcp_servers,
        skills: config.skills,
        system_prompt,
    })
}

pub fn validate_agent_config(config_path: &Path, dir_name: &str) -> Result<()> {
    let config_text = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let config: DiskAgentConfig = serde_yaml::from_str(&config_text)
        .with_context(|| format!("Invalid YAML in {}", config_path.display()))?;
    config.validate(dir_name)?;
    Ok(())
}

pub fn get_agents_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.agents()))
}

pub fn scan_agent_dirs(agents_path: &Path) -> Result<Vec<(String, std::path::PathBuf)>> {
    if !agents_path.exists() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();

    for entry in std::fs::read_dir(agents_path)? {
        let entry = entry?;
        let agent_path = entry.path();

        if !agent_path.is_dir() {
            continue;
        }

        let config_path = agent_path.join(AGENT_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }

        let dir_name = agent_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(ToString::to_string);

        if let Some(name) = dir_name {
            dirs.push((name, agent_path));
        }
    }

    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(dirs)
}
