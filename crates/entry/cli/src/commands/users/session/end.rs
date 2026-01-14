use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::{UserAdminService, UserService};
use systemprompt_identifiers::SessionId;
use systemprompt_runtime::AppContext;

use crate::commands::users::types::SessionEndOutput;

#[derive(Debug, Args)]
pub struct EndArgs {
    /// Session ID to end (optional if using --user --all)
    pub session_id: Option<String>,

    /// User identifier (ID, email, or username) to end sessions for
    #[arg(long)]
    pub user: Option<String>,

    /// End all sessions for the specified user (requires --user)
    #[arg(long)]
    pub all: bool,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: EndArgs, config: &CliConfig) -> Result<()> {
    if !args.yes {
        CliService::warning("This will end user session(s). Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let ctx = AppContext::new().await?;
    let user_service = UserService::new(ctx.db_pool())?;
    let admin_service = UserAdminService::new(user_service.clone());

    // Case 1: End all sessions for a user
    if args.all {
        let user_identifier = args.user.ok_or_else(|| {
            anyhow!("--user is required when using --all")
        })?;

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

        if config.is_json_output() {
            CliService::json(&output);
        } else if count > 0 {
            CliService::success(&output.message);
        } else {
            CliService::info("No active sessions to end");
        }

        return Ok(());
    }

    // Case 2: End a specific session
    let session_id_str = args.session_id.ok_or_else(|| {
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

    if config.is_json_output() {
        CliService::json(&output);
    } else if ended {
        CliService::success(&output.message);
    } else {
        CliService::warning(&output.message);
    }

    Ok(())
}
