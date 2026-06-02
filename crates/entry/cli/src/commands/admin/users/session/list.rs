use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::{UserAdminService, UserService};

use crate::CliConfig;
use crate::commands::admin::users::types::{SessionListOutput, SessionSummary};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(value_name = "USER_ID")]
    pub user: String,

    #[arg(long)]
    pub active: bool,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}

pub(super) async fn execute(args: ListArgs, config: &CliConfig) -> Result<CommandOutput> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub(super) async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
    let user_service = UserService::new(pool)?;
    let admin_service = UserAdminService::new(user_service.clone());

    let existing = admin_service.find_user(&args.user).await?;
    let Some(user) = existing else {
        return Err(anyhow!("User not found: {}", args.user));
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

    Ok(CommandOutput::table_of(
        vec![
            "session_id",
            "ip_address",
            "device_type",
            "started_at",
            "is_active",
        ],
        &output.sessions,
    )
    .with_title("User Sessions"))
}
