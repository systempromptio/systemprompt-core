use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use super::types::AgentEditOutput;
use systemprompt_core_logging::CliService;
use systemprompt_loader::ConfigLoader;

#[derive(Args)]
pub struct EditArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(long = "set", value_name = "KEY=VALUE", help = "Set a configuration value")]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the agent", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the agent", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long, help = "Set the port")]
    pub port: Option<u16>,

    #[arg(long, help = "Set the AI provider")]
    pub provider: Option<String>,

    #[arg(long, help = "Set the AI model")]
    pub model: Option<String>,
}

pub async fn execute(
    args: EditArgs,
    config: &CliConfig,
) -> Result<CommandResult<AgentEditOutput>> {
    let services_config = ConfigLoader::load()
        .context("Failed to load services configuration")?;

    let name = resolve_input(
        args.name,
        "name",
        config,
        || prompt_agent_selection(&services_config),
    )?;

    let _agent = services_config
        .agents
        .get(&name)
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

    let mut changes = Vec::new();

    if args.enable {
        changes.push("enabled: true".to_string());
    }

    if args.disable {
        changes.push("enabled: false".to_string());
    }

    if let Some(port) = args.port {
        changes.push(format!("port: {}", port));
    }

    if let Some(provider) = &args.provider {
        changes.push(format!("metadata.provider: {}", provider));
    }

    if let Some(model) = &args.model {
        changes.push(format!("metadata.model: {}", model));
    }

    for set_value in &args.set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid --set format: '{}'. Expected key=value", set_value));
        }
        changes.push(format!("{}: {}", parts[0], parts[1]));
    }

    if changes.is_empty() {
        return Err(anyhow!(
            "No changes specified. Use --enable, --disable, --port, --provider, --model, or --set key=value"
        ));
    }

    CliService::warning(
        "Agent editing modifies configuration files. \
         Please update the agent configuration in your services.yaml manually for now."
    );

    let output = AgentEditOutput {
        name: name.clone(),
        message: format!(
            "Agent '{}' edit prepared. Apply the following changes to services.yaml:\n\n{}",
            name,
            changes.join("\n")
        ),
        changes,
    };

    Ok(CommandResult::text(output).with_title(&format!("Edit Agent: {}", name)))
}

fn prompt_agent_selection(
    config: &systemprompt_models::ServicesConfig,
) -> Result<String> {
    let mut agents: Vec<&String> = config.agents.keys().collect();
    agents.sort();

    if agents.is_empty() {
        return Err(anyhow!("No agents configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent to edit")
        .items(&agents)
        .default(0)
        .interact()
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
