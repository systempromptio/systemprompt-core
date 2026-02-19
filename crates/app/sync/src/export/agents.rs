use super::escape_yaml;
use anyhow::Result;
use std::fs;
use std::path::Path;
use systemprompt_agent::models::Agent;

pub fn generate_agent_system_prompt(agent: &Agent) -> Option<String> {
    agent.system_prompt.as_ref().map(|sp| {
        format!(
            "---\ndescription: \"{}\"\n---\n\n{}",
            escape_yaml(&agent.description),
            sp
        )
    })
}

pub fn generate_agent_config(agent: &Agent) -> String {
    let tags_yaml = if agent.tags.is_empty() {
        "[]".to_string()
    } else {
        agent
            .tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mcp_servers_yaml = if agent.mcp_servers.is_empty() {
        "[]".to_string()
    } else {
        agent
            .mcp_servers
            .iter()
            .map(|s| format!("  - {}", s))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let skills_yaml = if agent.skills.is_empty() {
        "[]".to_string()
    } else {
        agent
            .skills
            .iter()
            .map(|s| format!("  - {}", s))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"id: {}
name: "{}"
display_name: "{}"
description: "{}"
version: "{}"
enabled: {}
port: {}
tags:
{}
mcp_servers:
{}
skills:
{}"#,
        agent.agent_id.as_str(),
        escape_yaml(&agent.name),
        escape_yaml(&agent.display_name),
        escape_yaml(&agent.description),
        escape_yaml(&agent.version),
        agent.enabled,
        agent.port,
        tags_yaml,
        mcp_servers_yaml,
        skills_yaml
    )
}

pub fn export_agent_to_disk(agent: &Agent, base_path: &Path) -> Result<()> {
    let agent_dir = base_path.join(&agent.name);
    fs::create_dir_all(&agent_dir)?;

    let config_content = generate_agent_config(agent);
    fs::write(agent_dir.join("config.yaml"), config_content)?;

    if let Some(system_prompt) = generate_agent_system_prompt(agent) {
        fs::write(agent_dir.join("system_prompt.md"), system_prompt)?;
    }

    Ok(())
}
