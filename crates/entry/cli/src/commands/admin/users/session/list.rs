use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService};

use crate::commands::admin::users::types::{SessionListOutput, SessionSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    pub user_id: String,

    #[arg(long)]
    pub active: bool,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}

pub async fn execute(
    args: ListArgs,
    config: &CliConfig,
) -> Result<CommandResult<SessionListOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<SessionListOutput>> {
    let user_service = UserService::new(pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let existing = admin_service.find_user(&args.user_id).await?;
    let Some(user) = existing else {
        return Err(anyhow!("User not found: {}", args.user_id));
    };

    let sessions = if args.active {
        user_service.list_active_sessions(&user.id).await?
    } else {
        user_service
            .list_recent_sessions(&user.id, args.limit)
            .await?
    };

    let summaries: Vec<SessionSummary> = sessions
        .into_iter()
        .map(|s| SessionSummary {
            session_id: s.session_id.clone(),
            ip_address: s.ip_address,
            user_agent: s.user_agent,
            device_type: s.device_type,
            started_at: s.started_at,
            last_activity_at: s.last_activity_at,
            is_active: s.ended_at.is_none(),
        })
        .collect();

    let output = SessionListOutput {
        total: summaries.len(),
        sessions: summaries,
    };

    Ok(CommandResult::table(output)
        .with_title("User Sessions")
        .with_columns(vec![
            "session_id".to_string(),
            "ip_address".to_string(),
            "device_type".to_string(),
            "started_at".to_string(),
            "is_active".to_string(),
        ]))
}
