pub mod list;
pub mod sync;
pub mod types;

use anyhow::Result;
use clap::Subcommand;

use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum PlaybooksCommands {
    #[command(about = "List playbooks")]
    List(list::ListArgs),

    #[command(about = "Sync playbooks between disk and database")]
    Sync(sync::SyncArgs),
}

pub async fn execute(cmd: PlaybooksCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        PlaybooksCommands::List(args) => {
            let result = list::execute(args).await?;
            result.print(config.output_format());
            Ok(())
        },
        PlaybooksCommands::Sync(args) => {
            let result = sync::execute(args, config).await?;
            result.print(config.output_format());
            Ok(())
        },
    }
}
