use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService};

use super::types::{SessionSummary, UserActivityOutput, UserDetailOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ShowArgs {
    pub identifier: String,

    #[arg(long)]
    pub sessions: bool,

    #[arg(long)]
    pub activity: bool,
}

pub async fn execute(
    args: ShowArgs,
    config: &CliConfig,
) -> Result<CommandResult<UserDetailOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ShowArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<UserDetailOutput>> {
    let user_service = UserService::new(pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let user = admin_service.find_user(&args.identifier).await?;

    let Some(user) = user else {
        return Err(anyhow!("User not found: {}", args.identifier));
    };

    let sessions = if args.sessions {
        let user_sessions = user_service.list_sessions(&user.id).await?;
        Some(
            user_sessions
                .into_iter()
                .map(|s| SessionSummary {
                    session_id: s.session_id,
                    ip_address: s.ip_address,
                    user_agent: s.user_agent,
                    device_type: s.device_type,
                    started_at: s.started_at,
                    last_activity_at: s.last_activity_at,
                    is_active: s.ended_at.is_none(),
                })
                .collect(),
        )
    } else {
        None
    };

    let activity = if args.activity {
        let user_activity = user_service.get_activity(&user.id).await?;
        Some(UserActivityOutput {
            user_id: user_activity.user_id,
            last_active: user_activity.last_active,
            session_count: user_activity.session_count,
            task_count: user_activity.task_count,
            message_count: user_activity.message_count,
        })
    } else {
        None
    };

    let output = UserDetailOutput {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        full_name: user.full_name.clone(),
        display_name: user.display_name.clone(),
        status: user.status.clone(),
        email_verified: user.email_verified,
        roles: user.roles.clone(),
        is_bot: user.is_bot,
        is_scanner: user.is_scanner,
        created_at: user.created_at,
        updated_at: user.updated_at,
        sessions,
        activity,
    };

    Ok(CommandResult::card(output).with_title(format!("User: {}", user.name)))
}
