mod ban;
mod count;
mod create;
mod delete;
mod list;
mod role;
mod search;
mod session;
mod show;
mod types;
mod update;

use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Subcommand;

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum UsersCommands {
    #[command(about = "List users with pagination and filtering")]
    List(list::ListArgs),

    #[command(about = "Show detailed user information")]
    Show(show::ShowArgs),

    #[command(about = "Search users by name, email, or full name")]
    Search(search::SearchArgs),

    #[command(about = "Create a new user")]
    Create(create::CreateArgs),

    #[command(about = "Update user fields")]
    Update(update::UpdateArgs),

    #[command(about = "Delete a user")]
    Delete(delete::DeleteArgs),

    #[command(about = "Get total user count")]
    Count,

    #[command(subcommand, about = "Role management commands")]
    Role(role::RoleCommands),

    #[command(subcommand, about = "Session management commands")]
    Session(session::SessionCommands),

    #[command(subcommand, about = "IP ban management commands")]
    Ban(ban::BanCommands),
}

pub async fn execute(cmd: UsersCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        UsersCommands::List(args) => list::execute(args, config).await,
        UsersCommands::Show(args) => show::execute(args, config).await,
        UsersCommands::Search(args) => search::execute(args, config).await,
        UsersCommands::Create(args) => create::execute(args, config).await,
        UsersCommands::Update(args) => update::execute(args, config).await,
        UsersCommands::Delete(args) => delete::execute(args, config).await,
        UsersCommands::Count => count::execute(config).await,
        UsersCommands::Role(cmd) => role::execute(cmd, config).await,
        UsersCommands::Session(cmd) => session::execute(cmd, config).await,
        UsersCommands::Ban(cmd) => ban::execute(cmd, config).await,
    }
}
