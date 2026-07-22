//! `core skills show` command rendering one skill's detail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;

use crate::CliConfig;
use crate::shared::CommandOutput;

use super::list::show_skill_detail;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Skill ID (directory name)")]
    pub name: String,
}

pub(super) fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let skills_path = get_skills_path()?;
    show_skill_detail(&args.name, &skills_path)
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_config::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}
