pub mod stream;
pub mod trace;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum LogsCommands {
    #[command(about = "Stream logs from database")]
    Stream(stream::StreamArgs),

    #[command(subcommand, about = "Trace message execution")]
    Trace(trace::TraceCommands),
}

pub async fn execute(command: LogsCommands) -> Result<()> {
    match command {
        LogsCommands::Stream(args) => stream::execute(args).await,
        LogsCommands::Trace(cmd) => trace::execute(cmd).await,
    }
}
