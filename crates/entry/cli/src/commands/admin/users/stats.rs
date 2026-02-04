use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use super::types::UserStatsOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

pub async fn execute(config: &CliConfig) -> Result<CommandResult<UserStatsOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<UserStatsOutput>> {
    let user_service = UserService::new(pool)?;

    let stats = user_service.get_stats().await?;

    let output = UserStatsOutput {
        total: stats.total,
        created_24h: stats.created_24h,
        created_7d: stats.created_7d,
        created_30d: stats.created_30d,
        active: stats.active,
        suspended: stats.suspended,
        admins: stats.admins,
        anonymous: stats.anonymous,
        bots: stats.bots,
        oldest_user: stats.oldest_user,
        newest_user: stats.newest_user,
    };

    Ok(CommandResult::card(output).with_title("User Statistics"))
}
