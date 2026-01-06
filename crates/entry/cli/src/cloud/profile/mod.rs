mod api_keys;
mod builders;
mod create;
mod create_setup;
mod create_tenant;
mod delete;
mod edit;
mod edit_secrets;
mod edit_settings;
mod list;
mod show;
mod show_display;
mod show_types;
mod templates;

pub use api_keys::collect_api_keys;
pub use create::create_profile_for_tenant;

use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};

#[derive(Subcommand)]
pub enum ProfileCommands {
    #[command(about = "Create a new profile")]
    Create { name: String },

    #[command(about = "List all profiles")]
    List,

    #[command(about = "Show profile configuration")]
    Show {
        name: Option<String>,

        #[arg(short, long, value_enum, default_value = "all")]
        filter: ShowFilter,

        #[arg(long, help = "Output as JSON")]
        json: bool,

        #[arg(long, help = "Output as YAML")]
        yaml: bool,
    },

    #[command(about = "Delete a profile")]
    Delete { name: String },

    #[command(about = "Edit profile configuration")]
    Edit { name: Option<String> },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ShowFilter {
    All,
    Agents,
    Mcp,
    Skills,
    Ai,
    Web,
    Content,
    Env,
    Settings,
}

pub async fn execute(cmd: Option<ProfileCommands>) -> Result<()> {
    match cmd {
        Some(cmd) => execute_command(cmd).await.map(|_| ()),
        None => {
            loop {
                match select_operation()? {
                    Some(cmd) => {
                        if execute_command(cmd).await? {
                            break;
                        }
                    },
                    None => break,
                }
            }
            Ok(())
        },
    }
}

async fn execute_command(cmd: ProfileCommands) -> Result<bool> {
    match cmd {
        ProfileCommands::Create { name } => create::execute(&name).await.map(|_| true),
        ProfileCommands::List => list::execute().await.map(|_| false),
        ProfileCommands::Show {
            name,
            filter,
            json,
            yaml,
        } => show::execute(name.as_deref(), filter, json, yaml)
            .await
            .map(|_| false),
        ProfileCommands::Delete { name } => delete::execute(&name).await.map(|_| false),
        ProfileCommands::Edit { name } => edit::execute(name.as_deref()).await.map(|_| false),
    }
}

fn select_operation() -> Result<Option<ProfileCommands>> {
    let operations = ["Create", "List", "Edit", "Delete", "Done"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile operation")
        .items(&operations)
        .default(0)
        .interact()?;

    let cmd = match selection {
        0 => {
            let name: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Profile name")
                .interact_text()?;
            Some(ProfileCommands::Create { name })
        },
        1 => Some(ProfileCommands::List),
        2 => Some(ProfileCommands::Edit { name: None }),
        3 => {
            let name: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Profile name to delete")
                .interact_text()?;
            Some(ProfileCommands::Delete { name })
        },
        4 => None,
        _ => unreachable!(),
    };

    Ok(cmd)
}
