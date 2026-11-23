//! `admin agents logs` command dispatch. `--logs-dir` is confined to the
//! profile's logs root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use clap::Args;

use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_models::path_confine::logs::confined_logs_dir;

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

pub(super) async fn execute(args: LogsArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let prompter = host.prompter();
    let config = host.output();
    let logs_root = host
        .resolved()
        .await?
        .get::<Arc<AppPaths>>()?
        .system()
        .logs();
    let logs_path = confined_logs_dir(&logs_root, args.logs_dir.as_deref())?;

    if args.follow {
        return execute_follow_mode(&args, prompter, config, &logs_path, host.children());
    }

    if args.disk {
        return execute_disk_mode(&args, prompter, config, &logs_path);
    }

    match execute_db_mode(&args, host).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::debug!(error = %e, "DB log query failed, falling back to disk");
            execute_disk_mode(&args, prompter, config, &logs_path)
        },
    }
}
