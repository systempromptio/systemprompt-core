//! `plugins capabilities` subcommands.
//!
//! Aggregates what each admitted member contributes — jobs, schemas, roles
//! and configuration sections — either as a per-kind listing or as a single
//! rolled-up summary across the admitted composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod jobs;
pub mod roles;
pub mod schemas;

use clap::{Args, Subcommand};
use systemprompt_extension::cli::{CommandError, CommandHost, CommandOutput};

use super::composition;
use super::types::CapabilitiesSummaryOutput;

#[derive(Debug, Args)]
pub struct CapabilitiesArgs {
    #[command(subcommand)]
    pub cmd: Option<CapabilitiesCommands>,
}

#[derive(Debug, Subcommand)]
pub enum CapabilitiesCommands {
    #[command(about = "List all jobs across extensions")]
    Jobs(jobs::JobsArgs),

    #[command(about = "List all schemas across extensions")]
    Schemas(schemas::SchemasArgs),

    #[command(about = "List all roles across extensions")]
    Roles(roles::RolesArgs),
}

pub(super) fn execute(
    args: CapabilitiesArgs,
    host: &dyn CommandHost,
) -> Result<CommandOutput, CommandError> {
    let built = composition::built(host)?;
    Ok(match args.cmd {
        None => execute_summary(built),
        Some(CapabilitiesCommands::Jobs(args)) => jobs::execute(&args, built),
        Some(CapabilitiesCommands::Schemas(args)) => schemas::execute(&args, built),
        Some(CapabilitiesCommands::Roles(args)) => roles::execute(&args, built),
    })
}

fn execute_summary(built: &systemprompt_extension::BuiltState) -> CommandOutput {
    let mut output = CapabilitiesSummaryOutput {
        jobs: 0,
        schemas: 0,
        roles: 0,
        configs: 0,
        extension_count: 0,
    };

    for member in built.members() {
        let capabilities = composition::capabilities(member);
        output.extension_count += 1;
        output.jobs += capabilities.jobs;
        output.schemas += capabilities.schemas;
        output.roles += capabilities.roles;
        output.configs += capabilities.configs;
    }

    CommandOutput::card_value("Capabilities Summary", &output)
}
