//! `plugins mcp logs` command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_models::path_confine::confine_within;

use super::app_paths;
use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};

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

    #[arg(
        long,
        help = "Logs directory inside the profile's logs root (relative paths resolve against it)"
    )]
    pub logs_dir: Option<String>,

    #[arg(
        long,
        value_enum,
        help = "Filter by log level: debug, info, warn, error"
    )]
    pub level: Option<LogLevel>,
}

pub fn confined_logs_dir(logs_root: &Path, requested: Option<&str>) -> Result<PathBuf> {
    let Some(dir) = requested else {
        return Ok(logs_root.to_path_buf());
    };
    let candidate = logs_root.join(dir);
    confine_within(logs_root, &candidate).with_context(|| {
        format!(
            "--logs-dir must name a directory inside {}",
            logs_root.display()
        )
    })
}

pub(super) async fn execute(args: LogsArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let prompter = host.prompter();
    let config = host.output();
    let logs_root = app_paths(host).await?.system().logs();
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
