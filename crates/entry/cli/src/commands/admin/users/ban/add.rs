//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_users::{BanDuration, BanIpParams, BannedIpRepository};

use crate::commands::admin::users::types::BanAddOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

const CLI_BAN_SOURCE: &str = "cli";

#[derive(Debug, Args)]
pub struct AddArgs {
    pub ip: String,

    #[arg(long)]
    pub reason: String,

    #[arg(long)]
    pub duration: Option<String>,

    #[arg(long)]
    pub permanent: bool,
}

pub(super) async fn execute(args: AddArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let pool = ctx.db_pool().await?;
    let ban_repository = BannedIpRepository::new(&pool)?;

    let duration = if args.permanent {
        BanDuration::Permanent
    } else {
        parse_duration(args.duration.as_deref())?
    };

    let params = BanIpParams::new(&args.ip, &args.reason, duration, CLI_BAN_SOURCE);

    ban_repository.ban_ip(params).await?;

    let expires_at = duration.to_expiry();

    let output = BanAddOutput {
        ip_address: args.ip.clone(),
        reason: args.reason.clone(),
        expires_at,
        is_permanent: args.permanent,
        message: format!("IP address '{}' has been banned", args.ip),
    };

    Ok(CommandOutput::card_value("IP Banned", &output))
}

fn parse_duration(duration_str: Option<&str>) -> Result<BanDuration> {
    let Some(s) = duration_str else {
        return Ok(BanDuration::Days(7));
    };

    let s = s.trim().to_lowercase();

    if s.ends_with('h') {
        let hours: i64 = s
            .trim_end_matches('h')
            .parse()
            .map_err(|_e| anyhow!("Invalid hours format"))?;
        Ok(BanDuration::Hours(hours))
    } else if s.ends_with('d') {
        let days: i64 = s
            .trim_end_matches('d')
            .parse()
            .map_err(|_e| anyhow!("Invalid days format"))?;
        Ok(BanDuration::Days(days))
    } else {
        Err(anyhow!(
            "Invalid duration format. Use format like '1h', '7d', '30d'"
        ))
    }
}
