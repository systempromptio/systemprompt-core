//! `skills` CLI command group: list and show configured skills.
//!
//! [`SkillsCommands`] enumerates the subcommands; [`execute`] resolves the
//! global config and dispatches via [`execute_with_config`]. Output payload
//! types live in [`types`].

pub mod types;

mod list;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::CliConfig;
use crate::cli_settings::get_global_config;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SkillsCommands {
    #[command(about = "List configured skills")]
    List(list::ListArgs),

    #[command(about = "Show skill details")]
    Show(show::ShowArgs),
}

pub fn execute(command: SkillsCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config)
}

pub fn execute_with_config(command: SkillsCommands, config: &CliConfig) -> Result<()> {
    match command {
        SkillsCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list skills")?;
            render_result(&result);
            Ok(())
        },
        SkillsCommands::Show(args) => {
            let result = show::execute(&args, config).context("Failed to show skill")?;
            render_result(&result);
            Ok(())
        },
    }
}
