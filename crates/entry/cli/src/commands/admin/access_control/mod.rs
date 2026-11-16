//! `admin access-control` subcommand: inspect and promote live RBAC rules.
//!
//! Exposes [`AccessControlCommands`] for exporting the current role rules as a
//! committable YAML baseline and linting the live access-control tables for
//! unknown entities or unreachable rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod export;
mod lint;

use crate::kernel::KernelHost;
use anyhow::Result;
use clap::{Args, Subcommand};

use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum AccessControlCommands {
    #[command(
        about = "Print current role rules as a YAML snippet for promotion to the committed \
                 baseline"
    )]
    ExportYaml(ExportYamlArgs),

    #[command(
        about = "Lint the live access-control tables for unknown entities and unreachable rules; \
                 exits non-zero on findings"
    )]
    Lint(LintArgs),
}

#[derive(Debug, Clone, Copy, Args)]
pub struct ExportYamlArgs;

#[derive(Debug, Clone, Copy, Args)]
pub struct LintArgs;

pub async fn execute(cmd: AccessControlCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        AccessControlCommands::ExportYaml(args) => {
            let result = export::run(args, host).await?;
            Ok(result)
        },
        AccessControlCommands::Lint(args) => {
            let (text, exit_nonzero) = lint::run(args, host).await?;
            let result = CommandOutput::text_titled("Access-control lint", text);
            if exit_nonzero {
                host.render(&result);
                anyhow::bail!("access-control lint failed; see report above");
            }
            Ok(result)
        },
    }
}
