//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, ValueEnum};

use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};
use crate::CliConfig;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::AppPaths;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn matches(&self, level_str: &str) -> bool {
        let level_upper = level_str.to_uppercase();
        match self {
            Self::Debug => true,
            Self::Info => !level_upper.contains("DEBUG"),
            Self::Warn => level_upper.contains("WARN") || level_upper.contains("ERROR"),
            Self::Error => level_upper.contains("ERROR"),
        }
    }
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(help = "MCP server name (optional - shows all MCP logs if not specified)")]
    pub server: Option<String>,

    #[arg(
        long,
        short = 'n',
        visible_alias = "tail",
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

    #[arg(
        long,
        value_enum,
        help = "Filter by log level: debug, info, warn, error"
    )]
    pub level: Option<LogLevel>,
}

fn get_default_logs_dir() -> PathBuf {
    ProfileBootstrap::get()
        .ok()
        .and_then(|p| AppPaths::from_profile(&p.paths).ok())
        .map_or_else(|| PathBuf::from("/var/log"), |paths| paths.system().logs())
}

pub(super) async fn execute(
    args: LogsArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let logs_path = args
        .logs_dir
        .as_ref()
        .map_or_else(get_default_logs_dir, PathBuf::from);

    if args.follow {
        return execute_follow_mode(&args, prompter, config, &logs_path);
    }

    if args.disk {
        return execute_disk_mode(&args, prompter, config, &logs_path);
    }

    match execute_db_mode(&args, config).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::debug!(error = %e, "DB log query failed, falling back to disk");
            execute_disk_mode(&args, prompter, config, &logs_path)
        },
    }
}
