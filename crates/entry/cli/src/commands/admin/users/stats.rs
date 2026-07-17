//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_database::DbPool;
use systemprompt_users::UserService;

use super::types::UserStatsOutput;
use crate::CliConfig;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

pub(super) async fn execute(ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(&ctx.db_pool().await?, &ctx.cli).await
}

pub(super) async fn execute_with_pool(pool: &DbPool, _config: &CliConfig) -> Result<CommandOutput> {
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

    Ok(CommandOutput::card_value("User Statistics", &output))
}
