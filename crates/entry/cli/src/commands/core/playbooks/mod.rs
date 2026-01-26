pub mod list;
pub mod sync;
pub mod types;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
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
            let result = list::execute(args).context("Failed to list playbooks")?;
            render_result(&result);
            Ok(())
        },
        PlaybooksCommands::Sync(args) => {
            let result = sync::execute(args, config)
                .await
                .context("Failed to sync playbooks")?;
            render_result(&result);
            Ok(())
        },
    }
}
