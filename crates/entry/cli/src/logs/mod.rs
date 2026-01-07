pub mod stream;
pub mod trace;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum LogsCommands {
    #[command(subcommand, about = "View and manage logs")]
    Stream(stream::LogCommands),

    #[command(subcommand, about = "Trace message execution")]
    Trace(trace::TraceCommands),
}

pub async fn execute(command: LogsCommands) -> Result<()> {
    match command {
        LogsCommands::Stream(cmd) => stream::execute(Some(cmd)).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd).await,
    }
}
