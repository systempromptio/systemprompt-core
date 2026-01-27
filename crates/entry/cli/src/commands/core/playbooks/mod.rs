mod create;
mod delete;
mod edit;
pub mod list;
pub mod show;
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

    #[command(about = "Show full playbook content")]
    Show(show::ShowArgs),

    #[command(about = "Create new playbook")]
    Create(create::CreateArgs),

    #[command(about = "Edit playbook configuration")]
    Edit(edit::EditArgs),

    #[command(about = "Delete a playbook")]
    Delete(delete::DeleteArgs),

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
        PlaybooksCommands::Show(args) => {
            let result = show::execute(&args).context("Failed to show playbook")?;
            render_result(&result);
            Ok(())
        },
        PlaybooksCommands::Create(args) => {
            let result = create::execute(args, config)
                .await
                .context("Failed to create playbook")?;
            render_result(&result);
            Ok(())
        },
        PlaybooksCommands::Edit(args) => {
            let result = edit::execute(&args, config).context("Failed to edit playbook")?;
            render_result(&result);
            Ok(())
        },
        PlaybooksCommands::Delete(args) => {
            let result = delete::execute(args, config).context("Failed to delete playbook")?;
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
