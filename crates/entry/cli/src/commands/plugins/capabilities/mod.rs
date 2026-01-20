mod jobs;
mod llm_providers;
mod roles;
mod schemas;
mod templates;
mod tools;

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

pub fn execute(cmd: CapabilitiesCommands, config: &CliConfig) {
    match cmd {
        CapabilitiesCommands::Jobs(args) => {
            render_result(&jobs::execute(&args, config));
        },
        CapabilitiesCommands::Templates(args) => {
            render_result(&templates::execute(&args, config));
        },
        CapabilitiesCommands::Schemas(args) => {
            render_result(&schemas::execute(&args, config));
        },
        CapabilitiesCommands::Tools(args) => {
            render_result(&tools::execute(&args, config));
        },
        CapabilitiesCommands::Roles(args) => {
            render_result(&roles::execute(&args, config));
        },
        CapabilitiesCommands::LlmProviders(args) => {
            render_result(&llm_providers::execute(&args, config));
        },
    }
}
