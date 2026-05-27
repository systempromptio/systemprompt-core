mod export;
mod lint;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::CliConfig;
use crate::shared::{CommandResult, render_result};

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

pub async fn execute(cmd: AccessControlCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        AccessControlCommands::ExportYaml(args) => {
            let result = export::run(args, config).await?;
            render_result(&result);
            Ok(())
        },
        AccessControlCommands::Lint(args) => {
            let (text, exit_nonzero) = lint::run(args, config).await?;
            let result = CommandResult::raw_text(text).with_title("Access-control lint");
            render_result(&result);
            if exit_nonzero {
                anyhow::bail!("access-control lint failed; see report above");
            }
            Ok(())
        },
    }
}
