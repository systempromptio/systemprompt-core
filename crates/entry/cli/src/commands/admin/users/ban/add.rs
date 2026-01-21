use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_users::{BanDuration, BanIpParams, BannedIpRepository};

use crate::commands::admin::users::types::BanAddOutput;

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

pub async fn execute(args: AddArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let ban_repository = BannedIpRepository::new(ctx.db_pool())?;

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

    if config.is_json_output() {
        CliService::json(&output);
    } else {
        CliService::success(&output.message);
        CliService::key_value("IP", &output.ip_address);
        CliService::key_value("Reason", &output.reason);
        match output.expires_at {
            Some(expires) => CliService::key_value("Expires", &expires.to_rfc3339()),
            None => CliService::key_value("Expires", "Never (permanent)"),
        }
    }

    Ok(())
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
            .map_err(|_| anyhow!("Invalid hours format"))?;
        Ok(BanDuration::Hours(hours))
    } else if s.ends_with('d') {
        let days: i64 = s
            .trim_end_matches('d')
            .parse()
            .map_err(|_| anyhow!("Invalid days format"))?;
        Ok(BanDuration::Days(days))
    } else {
        Err(anyhow!(
            "Invalid duration format. Use format like '1h', '7d', '30d'"
        ))
    }
}
