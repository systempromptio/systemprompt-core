pub mod stream;
pub mod trace;

use anyhow::Result;
use clap::Subcommand;

use crate::CliConfig;

#[derive(Subcommand)]
pub enum LogsCommands {
    #[command(subcommand, about = "View and manage logs")]
    Stream(stream::StreamCommands),

    #[command(subcommand, about = "Trace message execution")]
    Trace(trace::TraceCommands),
}

pub async fn execute(command: LogsCommands, config: &CliConfig) -> Result<()> {
    match command {
        LogsCommands::Stream(cmd) => stream::execute(cmd, config).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd, config).await,
    }
}
