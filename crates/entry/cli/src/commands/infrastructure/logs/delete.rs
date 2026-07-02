use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_logging::LoggingMaintenanceService;

use super::LogDeleteOutput;
use crate::context::CommandContext;
use crate::interactive::require_confirmation;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Clone, Copy, Args)]
pub struct DeleteArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub(super) async fn execute(args: DeleteArgs, ctx: &CommandContext) -> Result<()> {
    require_confirmation(
        ctx.prompter(),
        "Delete ALL log entries? This cannot be undone.",
        args.yes,
        &ctx.cli,
    )?;

    let service = LoggingMaintenanceService::new(&ctx.db_pool().await?)?;

    let deleted_count = service
        .clear_all_logs()
        .await
        .map_err(|e| anyhow!("Failed to delete logs: {}", e))?;

    let output = LogDeleteOutput {
        deleted_count,
        vacuum_performed: false,
    };

    let result = CommandOutput::card_value("Logs Deleted", &output);

    render_result(&result, &ctx.cli);

    Ok(())
}
