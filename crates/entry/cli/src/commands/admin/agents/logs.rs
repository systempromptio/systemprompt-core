use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};
use super::types::AgentLogsOutput;
use crate::CliConfig;
use crate::shared::CommandResult;

const DEFAULT_LOGS_DIR: &str = "/var/www/html/tyingshoelaces/logs";

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

    #[arg(long, help = "Custom logs directory path")]
    pub logs_dir: Option<String>,
}

pub async fn execute(args: LogsArgs, config: &CliConfig) -> Result<CommandResult<AgentLogsOutput>> {
    let logs_dir = args.logs_dir.as_deref().unwrap_or(DEFAULT_LOGS_DIR);
    let logs_path = PathBuf::from(logs_dir);

    if args.follow {
        return execute_follow_mode(&args, config, &logs_path);
    }

    if args.disk {
        return execute_disk_mode(&args, config, &logs_path);
    }

    match execute_db_mode(&args, config).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::debug!(error = %e, "DB log query failed, falling back to disk");
            execute_disk_mode(&args, config, &logs_path)
        },
    }
}
