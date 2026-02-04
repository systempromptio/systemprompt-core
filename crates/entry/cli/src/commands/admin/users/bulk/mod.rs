mod delete;
mod update;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum BulkCommands {
    #[command(about = "Bulk delete users by filter")]
    Delete(delete::DeleteArgs),

    #[command(about = "Bulk update user status by filter")]
    Update(update::UpdateArgs),
}

pub async fn execute(cmd: BulkCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        BulkCommands::Delete(args) => {
            let result = delete::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BulkCommands::Update(args) => {
            let result = update::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
