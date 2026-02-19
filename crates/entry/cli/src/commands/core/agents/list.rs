use anyhow::{anyhow, Result};
use clap::Args;
use std::path::Path;
use systemprompt_models::AGENT_CONFIG_FILENAME;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::types::{
    get_agents_path, parse_agent_from_config, AgentDetailOutput, AgentListOutput, AgentSummary,
    ListOrDetail,
};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Agent ID to show details (optional)")]
    pub name: Option<String>,

    #[arg(long, help = "Show only enabled agents")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled agents", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ListOrDetail>> {
    let agents_path = get_agents_path()?;

    if let Some(name) = args.name {
        return show_agent_detail(&name, &agents_path);
    }

    let agents = scan_agents(&agents_path)?;

    let filtered: Vec<AgentSummary> = agents
        .into_iter()
        .filter(|a| {
            if args.enabled {
                a.enabled
            } else if args.disabled {
                !a.enabled
            } else {
                true
            }
        })
        .collect();

    let output = AgentListOutput { agents: filtered };

    Ok(CommandResult::table(ListOrDetail::List(output))
        .with_title("Agents")
        .with_columns(vec![
            "agent_id".to_string(),
            "name".to_string(),
            "display_name".to_string(),
            "enabled".to_string(),
            "port".to_string(),
            "tags".to_string(),
        ]))
}

fn show_agent_detail(agent_id: &str, agents_path: &Path) -> Result<CommandResult<ListOrDetail>> {
    let agent_dir = agents_path.join(agent_id);

    if !agent_dir.exists() {
        return Err(anyhow!("Agent '{}' not found", agent_id));
    }

    let config_path = agent_dir.join(AGENT_CONFIG_FILENAME);

    if !config_path.exists() {
        return Err(anyhow!(
            "Agent '{}' has no {} file",
            agent_id,
            AGENT_CONFIG_FILENAME
        ));
    }

    let parsed = parse_agent_from_config(&config_path, &agent_dir)?;

    let system_prompt_preview = parsed
        .system_prompt
        .as_deref()
        .map(|s| {
            let preview: String = s.chars().take(200).collect();
            if s.len() > 200 {
                format!("{preview}...")
            } else {
                preview
            }
        })
        .unwrap_or_default();

    let output = AgentDetailOutput {
        agent_id: agent_id.to_string(),
        name: parsed.name,
        display_name: parsed.display_name,
        description: parsed.description,
        enabled: parsed.enabled,
        port: parsed.port,
        tags: parsed.tags,
        category: parsed.category,
        mcp_servers: parsed.mcp_servers,
        skills: parsed.skills,
        system_prompt_preview,
    };

    Ok(
        CommandResult::card(ListOrDetail::Detail(output))
            .with_title(format!("Agent: {agent_id}")),
    )
}

fn scan_agents(agents_path: &Path) -> Result<Vec<AgentSummary>> {
    if !agents_path.exists() {
        return Ok(Vec::new());
    }

    let mut agents = Vec::new();

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

        match parse_agent_from_config(&config_path, &agent_path) {
            Ok(parsed) => {
                let dir_name = agent_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("Invalid agent directory name"))?;

                agents.push(AgentSummary {
                    agent_id: dir_name.to_string(),
                    name: parsed.name,
                    display_name: parsed.display_name,
                    enabled: parsed.enabled,
                    port: parsed.port,
                    tags: parsed.tags,
                });
            },
            Err(e) => {
                tracing::warn!(
                    path = %agent_path.display(),
                    error = %e,
                    "Failed to parse agent"
                );
            },
        }
    }

    agents.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    Ok(agents)
}
