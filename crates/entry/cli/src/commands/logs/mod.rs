pub mod stream;
pub mod trace;

use anyhow::Result;
use clap::Subcommand;

use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum LogsCommands {
    #[command(subcommand, about = "Real-time log viewer")]
    Stream(stream::StreamCommands),

    #[command(subcommand, about = "Debug AI request traces")]
    Trace(trace::TraceCommands),
}

pub async fn execute(command: LogsCommands, config: &CliConfig) -> Result<()> {
    match command {
        LogsCommands::Stream(cmd) => stream::execute(cmd, config).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd, config).await,
    }
}
