use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_runtime::AppContext;
use systemprompt_users::BannedIpRepository;

use crate::commands::admin::users::types::BanRemoveOutput;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct RemoveArgs {
    pub ip: String,

    #[arg(short = 'y', long)]
    pub yes: bool,
}

pub async fn execute(
    args: RemoveArgs,
    _config: &CliConfig,
) -> Result<CommandResult<BanRemoveOutput>> {
    if !args.yes {
        return Err(anyhow!(
            "This will remove the IP ban. Use --yes to confirm."
        ));
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

    Ok(CommandResult::text(output).with_title("Ban Removed"))
}
