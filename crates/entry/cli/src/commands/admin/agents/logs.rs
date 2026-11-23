//! `admin agents logs` command dispatch. `--logs-dir` is confined to the
//! profile's logs root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;

use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use crate::shared::logs_dir::resolve_logs_dir;

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(help = "Agent name (optional - shows all agent logs if not specified)")]
    pub agent: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "50",
        help = "Number of lines to show"
    )]
    pub lines: usize,

    #[arg(long, short, help = "Follow log output continuously (disk only)")]
    pub follow: bool,

    #[arg(long, help = "Force reading from disk files instead of database")]
    pub disk: bool,

    #[arg(
        long,
        help = "Logs directory inside the profile's logs root (relative paths resolve against it)"
    )]
    pub logs_dir: Option<String>,
}

pub(super) async fn execute(args: LogsArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let prompter = ctx.prompter();
    let config = &ctx.cli;
    let logs_path = resolve_logs_dir(ctx, args.logs_dir.as_deref())?;

    if args.follow {
        return execute_follow_mode(
            &args,
            prompter,
            config,
            &logs_path,
            ctx.foreground_children(),
        );
    }

    if args.disk {
        return execute_disk_mode(&args, prompter, config, &logs_path);
    }

    match execute_db_mode(&args, ctx).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::debug!(error = %e, "DB log query failed, falling back to disk");
            execute_disk_mode(&args, prompter, config, &logs_path)
        },
    }
}
