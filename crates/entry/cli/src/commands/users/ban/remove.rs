use crate::cli_settings::CliConfig;
use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_core_users::BannedIpRepository;
use systemprompt_runtime::AppContext;

use crate::commands::users::types::BanRemoveOutput;

#[derive(Debug, Args)]
pub struct RemoveArgs {
    pub ip: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(args: RemoveArgs, config: &CliConfig) -> Result<()> {
    if !args.yes {
        CliService::warning("This will remove the IP ban. Use --yes to confirm.");
        return Err(anyhow!("Operation cancelled - confirmation required"));
    }

    let ctx = AppContext::new().await?;
    let ban_repository = BannedIpRepository::new(ctx.db_pool())?;

    let removed = ban_repository.unban_ip(&args.ip).await?;

    let output = BanRemoveOutput {
        ip_address: args.ip.clone(),
        removed,
        message: if removed {
            format!("IP address '{}' has been unbanned", args.ip)
        } else {
            format!("IP address '{}' was not banned", args.ip)
        },
    };

    if config.is_json_output() {
        CliService::json(&output);
    } else if removed {
        CliService::success(&output.message);
    } else {
        CliService::warning(&output.message);
    }

    Ok(())
}
