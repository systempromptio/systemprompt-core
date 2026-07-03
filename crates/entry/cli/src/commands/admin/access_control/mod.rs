//! `admin access-control` subcommand: inspect and promote live RBAC rules.
//!
//! Exposes [`AccessControlCommands`] for exporting the current role rules as a
//! committable YAML baseline and linting the live access-control tables for
//! unknown entities or unreachable rules.

pub mod export;
mod lint;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::context::CommandContext;
use crate::shared::{CommandOutput, render_result};

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

pub async fn execute(cmd: AccessControlCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        AccessControlCommands::ExportYaml(args) => {
            let result = export::run(args, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AccessControlCommands::Lint(args) => {
            let (text, exit_nonzero) = lint::run(args, &ctx.cli).await?;
            let result = CommandOutput::text_titled("Access-control lint", text);
            render_result(&result, &ctx.cli);
            if exit_nonzero {
                anyhow::bail!("access-control lint failed; see report above");
            }
            Ok(())
        },
    }
}
