//! `core skills show` command rendering one skill's detail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use clap::Args;
use systemprompt_identifiers::SkillId;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Skill ID (directory name)", value_parser = crate::shared::parse_skill_id)]
    pub name: SkillId,
}
