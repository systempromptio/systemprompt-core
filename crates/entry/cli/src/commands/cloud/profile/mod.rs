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

use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

#[derive(Debug, Subcommand)]
pub enum ProfileCommands {
    #[command(about = "Create a new profile")]
    Create { name: String },

    #[command(about = "List all profiles")]
    List,

    #[command(
        about = "Show profile configuration",
        after_help = "EXAMPLES:\n  systemprompt cloud profile show\n  systemprompt cloud profile \
                      show --filter agents\n  systemprompt cloud profile show --json"
    )]
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
    Delete(DeleteArgs),

    #[command(about = "Edit profile configuration")]
    Edit { name: Option<String> },
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub name: String,

    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
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

pub async fn execute(cmd: Option<ProfileCommands>, config: &CliConfig) -> Result<()> {
    if let Some(cmd) = cmd {
        execute_command(cmd, config).await.map(drop)
    } else {
        if !config.is_interactive() {
            return Err(anyhow::anyhow!(
                "Profile subcommand required in non-interactive mode"
            ));
        }
        while let Some(cmd) = select_operation()? {
            if execute_command(cmd, config).await? {
                break;
            }
        }
        Ok(())
    }
}

async fn execute_command(cmd: ProfileCommands, config: &CliConfig) -> Result<bool> {
    match cmd {
        ProfileCommands::Create { name } => create::execute(&name, config).await.map(|()| true),
        ProfileCommands::List => list::execute(config).map(|()| false),
        ProfileCommands::Show {
            name,
            filter,
            json,
            yaml,
        } => show::execute(name.as_deref(), filter, json, yaml, config).map(|()| false),
        ProfileCommands::Delete(args) => delete::execute(&args, config).map(|()| false),
        ProfileCommands::Edit { name } => {
            edit::execute(name.as_deref(), config).await.map(|()| false)
        },
    }
}

fn select_operation() -> Result<Option<ProfileCommands>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();
    let has_profiles = profiles_dir.exists()
        && std::fs::read_dir(&profiles_dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
            })
            .unwrap_or(false);

    let edit_label = if has_profiles {
        "Edit".to_string()
    } else {
        "Edit (unavailable - no profiles)".to_string()
    };
    let delete_label = if has_profiles {
        "Delete".to_string()
    } else {
        "Delete (unavailable - no profiles)".to_string()
    };

    let operations = vec![
        "Create".to_string(),
        "List".to_string(),
        edit_label,
        delete_label,
        "Done".to_string(),
    ];

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
        2 | 3 if !has_profiles => {
            CliService::warning("No profiles found");
            CliService::info("Run 'systemprompt cloud profile create <name>' to create one.");
            return Ok(Some(ProfileCommands::List));
        },
        2 => Some(ProfileCommands::Edit { name: None }),
        3 => select_profile("Select profile to delete")?
            .map(|name| ProfileCommands::Delete(DeleteArgs { name, yes: false })),
        4 => None,
        _ => unreachable!(),
    };

    Ok(cmd)
}

fn select_profile(prompt: &str) -> Result<Option<String>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        CliService::warning("No profiles directory found.");
        return Ok(None);
    }

    let profiles: Vec<String> = std::fs::read_dir(&profiles_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    if profiles.is_empty() {
        CliService::warning("No profiles found.");
        return Ok(None);
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&profiles)
        .default(0)
        .interact()?;

    Ok(Some(profiles[selection].clone()))
}
