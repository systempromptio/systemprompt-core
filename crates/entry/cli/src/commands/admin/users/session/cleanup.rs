//! `admin users session cleanup` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::UserService;

use crate::commands::admin::users::types::SessionCleanupOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupArgs {
    #[arg(long, default_value = "30")]
    pub days: i32,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: CleanupArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;

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
