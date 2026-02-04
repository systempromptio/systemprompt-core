use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::{BanCheckOutput, BanSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct CheckArgs {
    pub ip: String,
}

pub async fn execute(args: CheckArgs, config: &CliConfig) -> Result<CommandResult<BanCheckOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: CheckArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<BanCheckOutput>> {
    let ban_repository = BannedIpRepository::new(pool)?;

    let is_banned = ban_repository.is_banned(&args.ip).await?;
    let ban_info = if is_banned {
        ban_repository
            .find_ban(&args.ip)
            .await?
            .map(|b| BanSummary {
                ip_address: b.ip_address,
                reason: b.reason,
                banned_at: b.banned_at,
                expires_at: b.expires_at,
                is_permanent: b.is_permanent,
                ban_count: b.ban_count,
                ban_source: b.ban_source,
            })
    } else {
        None
    };

    let output = BanCheckOutput {
        ip_address: args.ip.clone(),
        is_banned,
        ban_info,
    };

    Ok(CommandResult::card(output).with_title("Ban Check"))
}
