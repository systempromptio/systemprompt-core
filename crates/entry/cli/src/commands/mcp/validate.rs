//! Validate MCP connection

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::sync::Arc;

use super::types::McpValidateOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// MCP server name to validate
    #[arg(help = "MCP server name (required in non-interactive mode)")]
    pub service: Option<String>,
}

pub async fn execute(
    args: ValidateArgs,
    config: &CliConfig,
) -> Result<CommandResult<McpValidateOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let service = resolve_input(args.service, "service", config, || {
        prompt_server_selection(&services_config)
    })?;

    if !services_config.mcp_servers.contains_key(&service) {
        return Err(anyhow!("MCP server '{}' not found", service));
    }

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let manager = McpManager::new(ctx).context("Failed to initialize MCP manager")?;

    let (valid, tools_count, issues) = match manager.validate_service(&service).await {
        Ok(()) => (true, 0, vec![]),
        Err(e) => (false, 0, vec![e.to_string()]),
    };

    let output = McpValidateOutput {
        server: service,
        valid,
        tools_count,
        issues,
    };

    Ok(CommandResult::card(output).with_title("MCP Validation Result"))
}

fn prompt_server_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut servers: Vec<&String> = config.mcp_servers.keys().collect();
    servers.sort();

    if servers.is_empty() {
        return Err(anyhow!("No MCP servers configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select MCP server to validate")
        .items(&servers)
        .default(0)
        .interact()
        .context("Failed to get server selection")?;

    Ok(servers[selection].clone())
}
