//! `admin users session cleanup` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Result, anyhow};
use clap::Args;
use std::sync::Arc;
use systemprompt_users::{UserRepository, UserService};

use crate::commands::admin::users::types::SessionCleanupOutput;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(long, default_value = "30")]
    pub days: i32,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: CleanupArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let pool = systemprompt_database::pool_from_handle(host.db().await?)?;
    let user_service = UserService::new(Arc::new(UserRepository::new(&pool)));

    if !args.yes {
        return Err(anyhow!(
            "This will delete anonymous users older than {} days. Use --yes to confirm.",
            args.days
        ));
    }

    let cleaned = user_service.cleanup_old_anonymous(args.days).await?;

    let output = SessionCleanupOutput {
        cleaned,
        message: format!(
            "Cleaned up {} anonymous user(s) older than {} days",
            cleaned, args.days
        ),
    };

    Ok(CommandOutput::card_value("Session Cleanup", &output))
}
