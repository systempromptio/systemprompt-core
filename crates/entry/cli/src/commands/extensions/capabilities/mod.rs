mod jobs;
mod llm_providers;
mod roles;
mod schemas;
mod templates;
mod tools;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum CapabilitiesCommands {
    #[command(about = "List all jobs across extensions")]
    Jobs(jobs::JobsArgs),

    #[command(about = "List all templates across extensions")]
    Templates(templates::TemplatesArgs),

    #[command(about = "List all schemas across extensions")]
    Schemas(schemas::SchemasArgs),

    #[command(about = "List all tools across extensions")]
    Tools(tools::ToolsArgs),

    #[command(about = "List all roles across extensions")]
    Roles(roles::RolesArgs),

    #[command(about = "List all LLM providers across extensions")]
    LlmProviders(llm_providers::LlmProvidersArgs),
}

pub fn execute(cmd: CapabilitiesCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        CapabilitiesCommands::Jobs(args) => {
            let result = jobs::execute(args, config).context("Failed to list jobs")?;
            render_result(&result);
            Ok(())
        },
        CapabilitiesCommands::Templates(args) => {
            let result = templates::execute(args, config).context("Failed to list templates")?;
            render_result(&result);
            Ok(())
        },
        CapabilitiesCommands::Schemas(args) => {
            let result = schemas::execute(args, config).context("Failed to list schemas")?;
            render_result(&result);
            Ok(())
        },
        CapabilitiesCommands::Tools(args) => {
            let result = tools::execute(args, config).context("Failed to list tools")?;
            render_result(&result);
            Ok(())
        },
        CapabilitiesCommands::Roles(args) => {
            let result = roles::execute(args, config).context("Failed to list roles")?;
            render_result(&result);
            Ok(())
        },
        CapabilitiesCommands::LlmProviders(args) => {
            let result =
                llm_providers::execute(args, config).context("Failed to list LLM providers")?;
            render_result(&result);
            Ok(())
        },
    }
}
