//! `plugins mcp logs` command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, ValueEnum};

use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};
use crate::commands::shared::app_context;
use crate::shared::CommandOutput;

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

async fn default_logs_dir(host: &dyn KernelHost) -> Result<PathBuf> {
    Ok(app_context(host).await?.app_paths().system().logs())
}

pub(super) async fn execute(args: LogsArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let prompter = host.prompter();
    let config = host.output();
    let logs_path = match args.logs_dir.as_ref() {
        Some(dir) => PathBuf::from(dir),
        None => default_logs_dir(host).await?,
    };

    if args.follow {
        return execute_follow_mode(
            &args,
            prompter,
            config,
            &logs_path,
            host.children(),
        );
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
