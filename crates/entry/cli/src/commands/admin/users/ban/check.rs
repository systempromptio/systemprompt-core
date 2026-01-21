use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_logging::CliService;
use systemprompt_users::BannedIpRepository;
use systemprompt_runtime::AppContext;

use crate::commands::admin::users::types::{BanCheckOutput, BanSummary};

#[derive(Debug, Args)]
pub struct CheckArgs {
    pub ip: String,
}

pub async fn execute(args: CheckArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(args: CheckArgs, pool: &DbPool, config: &CliConfig) -> Result<()> {
    let ban_repository = BannedIpRepository::new(pool)?;

    let is_banned = ban_repository.is_banned(&args.ip).await?;
    let ban_info = if is_banned {
        ban_repository.get_ban(&args.ip).await?.map(|b| BanSummary {
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

    if config.is_json_output() {
        CliService::json(&output);
    } else if is_banned {
        CliService::warning(&format!("IP address '{}' is BANNED", args.ip));
        if let Some(ref info) = output.ban_info {
            CliService::key_value("Reason", &info.reason);
            CliService::key_value("Ban Count", &info.ban_count.to_string());
            CliService::key_value("Banned At", &info.banned_at.to_rfc3339());
            match info.expires_at {
                Some(ref expires) => CliService::key_value("Expires", &expires.to_rfc3339()),
                None => CliService::key_value("Expires", "Never (permanent)"),
            }
            if let Some(ref source) = info.ban_source {
                CliService::key_value("Source", source);
            }
        }
    } else {
        CliService::success(&format!("IP address '{}' is NOT banned", args.ip));
    }

    Ok(())
}
