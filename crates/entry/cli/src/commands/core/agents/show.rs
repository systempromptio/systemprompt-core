use anyhow::{Result, anyhow};
use clap::Args;
use std::path::Path;
use systemprompt_models::AGENT_CONFIG_FILENAME;

use crate::CliConfig;
use crate::shared::CommandResult;

use super::types::{AgentDetailOutput, get_agents_path, parse_agent_from_config};

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Agent ID (directory name)")]
    pub name: String,
}

pub fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandResult<AgentDetailOutput>> {
    let agents_path = get_agents_path()?;
    show_agent_detail(&args.name, &agents_path)
}

fn show_agent_detail(
    agent_id: &str,
    agents_path: &Path,
) -> Result<CommandResult<AgentDetailOutput>> {
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
        .map_or_else(String::new, |s| {
            let preview: String = s.chars().take(200).collect();
            if s.len() > 200 {
                format!("{preview}...")
            } else {
                preview
            }
        });

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

    Ok(CommandResult::card(output).with_title(format!("Agent: {agent_id}")))
}
