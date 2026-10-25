//! `admin agents validate` command checking card, provider, and MCP references.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;

use super::types::{ValidationIssue, ValidationOutput};
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use systemprompt_manifest::secrets::Secrets;
use systemprompt_manifest::services::catalog::{CatalogContext, ResolvedCatalog};
use systemprompt_manifest::{AgentConfig, ServicesConfig};

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "Agent name to validate (optional)")]
    pub name: Option<String>,
}

#[derive(Debug)]
pub struct ValidationSources<'a> {
    pub services_config: &'a ServicesConfig,
    pub catalog: &'a ResolvedCatalog,
    pub secrets: Option<&'a Secrets>,
}

pub(super) fn execute(args: &ValidateArgs, ctx: &CommandContext) -> Result<(CommandOutput, bool)> {
    let services_config = ctx
        .boot()?
        .load_services_config()
        .context("Failed to load services configuration")?;
    let catalog = services_config
        .resolve_catalog(&CatalogContext::default())
        .context("The services catalog does not resolve")?;
    let secrets = ctx.secrets()?.get();
    let sources = ValidationSources {
        services_config: &services_config,
        catalog: &catalog,
        secrets: Some(&secrets),
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut agents_checked = 0;

    let agents_to_check: Vec<(&String, &AgentConfig)> = match &args.name {
        Some(name) => {
            let agent = services_config
                .agents
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found", name))?;
            vec![(name, agent)]
        },
        None => services_config.agents.iter().collect(),
    };

    for (name, agent) in agents_to_check {
        agents_checked += 1;
        check_basics(name, agent, &mut errors, &mut warnings);
        check_provider(name, agent, &sources, &mut errors);
        check_mcp_references(name, agent, &services_config, &mut errors);
    }

    let valid = errors.is_empty();
    let output = ValidationOutput {
        valid,
        items_checked: agents_checked,
        errors,
        warnings,
    };

    Ok((
        CommandOutput::card_value("Validation Results", &output),
        valid,
    ))
}

pub fn check_basics(
    name: &str,
    agent: &AgentConfig,
    errors: &mut Vec<ValidationIssue>,
    warnings: &mut Vec<ValidationIssue>,
) {
    if let Err(e) = agent.validate(name) {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: e.to_string(),
            suggestion: None,
        });
    }

    if agent.port == 0 {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: "Port cannot be 0".to_owned(),
            suggestion: None,
        });
    }

    if agent.card.display_name.is_empty() {
        warnings.push(ValidationIssue {
            source: name.to_owned(),
            message: "Display name is empty".to_owned(),
            suggestion: None,
        });
    }

    if agent.card.description.is_empty() {
        warnings.push(ValidationIssue {
            source: name.to_owned(),
            message: "Description is empty".to_owned(),
            suggestion: None,
        });
    }

    if agent.enabled && agent.metadata.provider.is_none() {
        warnings.push(ValidationIssue {
            source: name.to_owned(),
            message: "Enabled agent has no AI provider configured".to_owned(),
            suggestion: None,
        });
    }
}

pub fn check_provider(
    name: &str,
    agent: &AgentConfig,
    sources: &ValidationSources<'_>,
    errors: &mut Vec<ValidationIssue>,
) {
    if !agent.enabled {
        return;
    }
    let Some(provider_name) = &agent.metadata.provider else {
        return;
    };

    let Some(provider) = sources.catalog.provider(provider_name) else {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: format!(
                "Provider '{provider_name}' is not declared in services/ai/providers.yaml"
            ),
            suggestion: None,
        });
        return;
    };

    if !provider.enabled {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: format!(
                "Provider '{provider_name}' is disabled in services/ai/providers.yaml (set \
                 enabled: true)"
            ),
            suggestion: None,
        });
    }

    let Some(secret_name) = sources
        .catalog
        .credential(provider.credential.as_str())
        .and_then(|credential| credential.platform_secret())
    else {
        return;
    };
    let key_present = sources
        .secrets
        .and_then(|s| s.get(secret_name.as_str()))
        .is_some_and(|k| !k.is_empty());
    if !key_present {
        errors.push(ValidationIssue {
            source: name.to_owned(),
            message: format!(
                "No API key configured for provider '{provider_name}' (credential '{}', secret \
                 '{}' not found)",
                provider.credential,
                secret_name.as_str()
            ),
            suggestion: None,
        });
    }
}

pub fn check_mcp_references(
    name: &str,
    agent: &AgentConfig,
    services_config: &ServicesConfig,
    errors: &mut Vec<ValidationIssue>,
) {
    for mcp_server in &agent.metadata.mcp_servers.include {
        if !services_config.mcp_servers.contains_key(mcp_server) {
            errors.push(ValidationIssue {
                source: name.to_owned(),
                message: format!("Referenced MCP server '{}' not found in config", mcp_server),
                suggestion: None,
            });
        }
    }
}
