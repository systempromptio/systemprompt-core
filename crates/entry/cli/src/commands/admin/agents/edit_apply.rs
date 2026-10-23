//! Applies flag-driven edits to an agent definition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_agent::services::config_authoring::{
    AgentCapabilityFlags, AgentCardEdit, AgentCardLinks, AgentConfigAuthoringService,
    AgentEditRequest, AgentModelChoice, AgentPlacement, MembershipEdit, SystemPromptSource,
};
use systemprompt_logging::CliService;

use super::edit::EditArgs;

fn edit_request(args: &EditArgs) -> AgentEditRequest {
    AgentEditRequest {
        enable: args.enable,
        disable: args.disable,
        port: args.agent.port,
        placement: AgentPlacement {
            endpoint: args.agent.endpoint.clone(),
            dev_only: args.agent.dev_only,
            is_primary: args.agent.is_primary,
            default: args.agent.default,
        },
        card: AgentCardEdit {
            display_name: args.agent.display_name.clone(),
            description: args.agent.description.clone(),
            links: AgentCardLinks {
                version: args.agent.version.clone(),
                icon_url: args.agent.icon_url.clone(),
                documentation_url: args.agent.documentation_url.clone(),
            },
        },
        capabilities: AgentCapabilityFlags {
            streaming: args.agent.streaming,
            push_notifications: args.agent.push_notifications,
            extended_agent_card: args.agent.extended_agent_card,
        },
        model: AgentModelChoice {
            provider: args.agent.provider.clone(),
            model: args.agent.model.clone(),
        },
        system_prompt: SystemPromptSource {
            inline: args.agent.system_prompt.clone(),
            file: args.agent.system_prompt_file.clone(),
        },
        mcp_servers: MembershipEdit {
            add: args.agent.mcp_servers.clone(),
            remove: args.remove_mcp_servers.clone(),
        },
        skills: MembershipEdit {
            add: args.agent.skills.clone(),
            remove: args.remove_skills.clone(),
        },
        set_values: args.set_values.clone(),
    }
}

pub(super) fn apply_enabled_flags(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
    AgentConfigAuthoringService::apply_enabled_flags(agent, &edit_request(args), changes);
}

pub(super) fn apply_runtime_fields(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) -> Result<()> {
    AgentConfigAuthoringService::apply_runtime_fields(agent, &edit_request(args), changes)?;
    Ok(())
}

pub(super) fn apply_card_fields(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
    AgentConfigAuthoringService::apply_card_fields(agent, &edit_request(args), changes);
}

pub(super) fn apply_capability_fields(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
    AgentConfigAuthoringService::apply_capability_fields(agent, &edit_request(args), changes);
}

pub(super) fn apply_metadata_fields(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) -> Result<()> {
    AgentConfigAuthoringService::apply_metadata_fields(agent, &edit_request(args), changes)?;
    Ok(())
}

pub(super) fn apply_mcp_server_changes(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    services_config: &systemprompt_manifest::ServicesConfig,
    changes: &mut Vec<String>,
) -> Result<()> {
    let skipped = AgentConfigAuthoringService::apply_mcp_server_changes(
        agent,
        &edit_request(args),
        services_config,
        changes,
    )?;
    for mcp_server in &skipped {
        CliService::warning(&format!(
            "MCP server '{}' not found in agent configuration, skipping removal",
            mcp_server
        ));
    }
    Ok(())
}

pub(super) fn apply_skill_changes(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
    let skipped =
        AgentConfigAuthoringService::apply_skill_changes(agent, &edit_request(args), changes);
    for skill in &skipped {
        CliService::warning(&format!(
            "Skill '{}' not found in agent configuration, skipping removal",
            skill
        ));
    }
}

pub(super) fn apply_set_value_changes(
    agent: &mut systemprompt_manifest::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) -> Result<()> {
    AgentConfigAuthoringService::apply_set_value_changes(agent, &edit_request(args), changes)?;
    Ok(())
}
