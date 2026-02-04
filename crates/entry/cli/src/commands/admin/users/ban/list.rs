use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::{BanListOutput, BanSummary};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "50")]
    pub limit: i64,

    #[arg(long)]
    pub source: Option<String>,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<CommandResult<BanListOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandResult<BanListOutput>> {
    let ban_repository = BannedIpRepository::new(pool)?;

    let bans = match args.source {
        Some(ref source) => {
            ban_repository
                .list_bans_by_source(source, args.limit)
                .await?
        },
        None => ban_repository.list_active_bans(args.limit).await?,
    };

    let summaries: Vec<BanSummary> = bans
        .into_iter()
        .map(|b| BanSummary {
            ip_address: b.ip_address,
            reason: b.reason,
            banned_at: b.banned_at,
            expires_at: b.expires_at,
            is_permanent: b.is_permanent,
            ban_count: b.ban_count,
            ban_source: b.ban_source,
        })
        .collect();

    let output = BanListOutput {
        total: summaries.len(),
        bans: summaries,
    };

    Ok(CommandResult::table(output)
        .with_title("Banned IPs")
        .with_columns(vec![
            "ip_address".to_string(),
            "reason".to_string(),
            "banned_at".to_string(),
            "expires_at".to_string(),
            "is_permanent".to_string(),
        ]))
}
