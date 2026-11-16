//! `admin users ban cleanup` command pruning expired bans.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::BanCleanupOutput;
use crate::shared::CommandOutput;
use systemprompt_extension::cli::CommandHost;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: CleanupArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let ban_repository = BannedIpRepository::new(&pool);

    if !args.yes {
        return Err(anyhow!(
            "This will delete all expired bans. Use --yes to confirm."
        ));
    }

    let cleaned = ban_repository.cleanup_expired().await?;

    let output = BanCleanupOutput {
        cleaned,
        message: format!("Cleaned up {} expired ban(s)", cleaned),
    };

    Ok(CommandOutput::card_value("Ban Cleanup", &output))
}
