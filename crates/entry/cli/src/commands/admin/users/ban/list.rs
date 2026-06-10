use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_users::BannedIpRepository;

use crate::CliConfig;
use crate::commands::admin::users::types::{BanListOutput, BanSummary};
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "50")]
    pub limit: i64,

    #[arg(long)]
    pub source: Option<String>,
}

pub(super) async fn execute(args: ListArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    execute_with_pool(args, &ctx.db_pool().await?, &ctx.cli).await
}

pub(super) async fn execute_with_pool(
    args: ListArgs,
    pool: &DbPool,
    _config: &CliConfig,
) -> Result<CommandOutput> {
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

    Ok(CommandOutput::table_of(
        vec![
            "ip_address",
            "reason",
            "banned_at",
            "expires_at",
            "is_permanent",
        ],
        &output.bans,
    )
    .with_title("Banned IPs"))
}
