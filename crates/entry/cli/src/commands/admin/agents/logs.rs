//! `admin agents logs` command dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use systemprompt_extension::cli::CommandOutput;
use super::logs_db::execute_db_mode;
use super::logs_disk::{execute_disk_mode, execute_follow_mode};

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

pub(super) async fn execute(args: LogsArgs, host: &dyn KernelHost) -> Result<CommandOutput> {
    let prompter = host.prompter();
    let config = host.output();
    let logs_path = match args.logs_dir.as_deref() {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(
            crate::commands::admin::state::config(host)
                .await
                .context("agent log directory requires an initialised profile")?
                .logs_path(),
        ),
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
