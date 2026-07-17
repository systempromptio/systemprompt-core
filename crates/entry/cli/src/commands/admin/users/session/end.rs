//! `admin users session end` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_identifiers::SessionId;
use systemprompt_users::{UserAdminService, UserService};

use crate::commands::admin::users::types::SessionEndOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct EndArgs {
    #[arg(
        value_name = "SESSION_ID",
        help = "Session ID to end (optional if using --user --all)"
    )]
    pub session: Option<String>,

    #[arg(
        long,
        help = "User identifier (ID, email, or username) to end sessions for"
    )]
    pub user: Option<String>,

    #[arg(
        long,
        help = "End all sessions for the specified user (requires --user)"
    )]
    pub all: bool,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub(super) async fn execute(args: EndArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    if !args.yes {
        return Err(anyhow!(
            "This will end user session(s). Use --yes to confirm."
        ));
    }

    let pool = ctx.db_pool().await?;
    let user_service = UserService::new(&pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    if args.all {
        let user_identifier = args
            .user
            .ok_or_else(|| anyhow!("--user is required when using --all"))?;

        let user = admin_service
            .find_user(&user_identifier)
            .await?
            .ok_or_else(|| anyhow!("User not found: {}", user_identifier))?;

        let count = user_service.end_all_sessions(&user.id).await?;

        let output = SessionEndOutput {
            ended: vec![format!("all sessions for user '{}'", user.name)],
            count,
            message: format!("{} session(s) ended for user '{}'", count, user.name),
        };

        return Ok(CommandOutput::card_value("Sessions Ended", &output));
    }

    let session_id_str = args.session.ok_or_else(|| {
        anyhow!("Session ID is required (or use --user --all to end all sessions for a user)")
    })?;

    let session_id = SessionId::new(&session_id_str);
    let ended = user_service.end_session(&session_id).await?;

    let output = SessionEndOutput {
        ended: if ended {
            vec![session_id_str.clone()]
        } else {
            vec![]
        },
        count: u64::from(ended),
        message: if ended {
            format!("Session '{}' ended successfully", session_id_str)
        } else {
            format!(
                "Session '{}' was not found or already ended",
                session_id_str
            )
        },
    };

    Ok(CommandOutput::card_value("Sessions Ended", &output))
}
