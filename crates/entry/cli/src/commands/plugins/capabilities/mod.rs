mod jobs;
mod llm_providers;
mod roles;
mod schemas;
mod templates;
mod tools;

use clap::{Args, Subcommand};
use systemprompt_extension::ExtensionRegistry;

use super::types::CapabilitiesSummaryOutput;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct CapabilitiesArgs {
    #[command(subcommand)]
    pub cmd: Option<CapabilitiesCommands>,
}

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

pub fn execute(args: CapabilitiesArgs, config: &CliConfig) {
    match args.cmd {
        None => {
            render_result(&execute_summary(config));
        },
        Some(CapabilitiesCommands::Jobs(args)) => {
            render_result(&jobs::execute(&args, config));
        },
        Some(CapabilitiesCommands::Templates(args)) => {
            render_result(&templates::execute(&args, config));
        },
        Some(CapabilitiesCommands::Schemas(args)) => {
            render_result(&schemas::execute(&args, config));
        },
        Some(CapabilitiesCommands::Tools(args)) => {
            render_result(&tools::execute(&args, config));
        },
        Some(CapabilitiesCommands::Roles(args)) => {
            render_result(&roles::execute(&args, config));
        },
        Some(CapabilitiesCommands::LlmProviders(args)) => {
            render_result(&llm_providers::execute(&args, config));
        },
    }
}

pub fn execute_summary(_config: &CliConfig) -> CommandResult<CapabilitiesSummaryOutput> {
    let registry = ExtensionRegistry::discover();

    let mut jobs = 0;
    let mut templates = 0;
    let mut schemas = 0;
    let mut tools = 0;
    let mut roles = 0;
    let mut llm_providers = 0;
    let mut extension_count = 0;

    for ext in registry.extensions() {
        extension_count += 1;
        jobs += ext.jobs().len();
        schemas += ext.schemas().len();
        roles += ext.roles().len();
        llm_providers += ext.llm_providers().len();
        tools += ext.tool_providers().len();

        for provider in ext.template_providers() {
            templates += provider.templates().len();
        }
    }

    let output = CapabilitiesSummaryOutput {
        jobs,
        templates,
        schemas,
        tools,
        roles,
        llm_providers,
        extension_count,
    };

    CommandResult::card(output).with_title("Capabilities Summary")
}
