use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_logging::LoggingMaintenanceService;
use systemprompt_runtime::AppContext;

use super::LogDeleteOutput;
use crate::interactive::require_confirmation;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct DeleteArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(args: DeleteArgs, config: &CliConfig) -> Result<()> {
    require_confirmation(
        "Delete ALL log entries? This cannot be undone.",
        args.yes,
        config,
    )?;

    let ctx = AppContext::new().await?;
    let service = LoggingMaintenanceService::new(ctx.db_pool())?;

    let deleted_count = service
        .clear_all_logs()
        .await
        .map_err(|e| anyhow!("Failed to delete logs: {}", e))?;

    let output = LogDeleteOutput {
        deleted_count,
        vacuum_performed: false,
    };

    let result = CommandResult::card(output).with_title("Logs Deleted");

    render_result(&result);

    Ok(())
}
