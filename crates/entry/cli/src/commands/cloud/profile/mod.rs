//! `cloud profile` subcommands for managing deployment profiles.
//!
//! Dispatches [`ProfileCommands`] to create, list, show, edit, and delete
//! profiles, and drives the interactive operation picker when no subcommand is
//! given.

mod api_keys;
mod args;
mod create;
mod create_setup;
mod create_tenant;
pub(super) mod delete;
mod edit;
mod edit_secrets;
mod edit_settings;
mod list;
mod profile_steps;
mod show;
mod show_display;
mod show_types;
pub mod templates;

pub use api_keys::collect_api_keys;
pub use args::{CreateArgs, DeleteArgs, EditArgs, ProfileCommands, ShowFilter, TenantTypeArg};
pub use create::{CreatedProfile, create_profile_for_tenant};
pub use create_setup::{get_cloud_user, handle_local_tenant_setup};

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::Result;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_logging::CliService;

pub async fn execute(cmd: Option<ProfileCommands>, ctx: &CommandContext) -> Result<()> {
    if let Some(cmd) = cmd {
        execute_command(cmd, ctx).await.map(drop)
    } else {
        if !ctx.cli.is_interactive() {
            return Err(anyhow::anyhow!(
                "Profile subcommand required in non-interactive mode"
            ));
        }
        while let Some(cmd) = select_operation()? {
            if execute_command(cmd, ctx).await? {
                break;
            }
        }
        Ok(())
    }
}

async fn execute_command(cmd: ProfileCommands, ctx: &CommandContext) -> Result<bool> {
    match cmd {
        ProfileCommands::Create(args) => create::execute(&args, &ctx.cli).await.map(|()| true),
        ProfileCommands::List => {
            let result = list::execute(ctx)?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        ProfileCommands::Show {
            name,
            filter,
            json,
            yaml,
        } => show::execute(name.as_deref(), filter, json, yaml, ctx).map(|()| false),
        ProfileCommands::Delete(args) => {
            let result = delete::execute(&args, &ctx.cli)?;
            render_result(&result, &ctx.cli);
            Ok(false)
        },
        ProfileCommands::Edit(args) => edit::execute(&args, ctx).map(|()| false),
    }
}

fn select_operation() -> Result<Option<ProfileCommands>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();
    let has_profiles = profiles_dir.exists()
        && std::fs::read_dir(&profiles_dir).is_ok_and(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
        });

    let edit_label = if has_profiles {
        "Edit".to_owned()
    } else {
        "Edit (unavailable - no profiles)".to_owned()
    };
    let delete_label = if has_profiles {
        "Delete".to_owned()
    } else {
        "Delete (unavailable - no profiles)".to_owned()
    };

    let operations = vec![
        "List".to_owned(),
        edit_label,
        delete_label,
        "Done".to_owned(),
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile operation")
        .items(&operations)
        .default(0)
        .interact()?;

    let cmd = match selection {
        0 => Some(ProfileCommands::List),
        1 | 2 if !has_profiles => {
            CliService::warning("No profiles found");
            CliService::info(
                "Run 'systemprompt cloud tenant create' (or 'just tenant') to create a tenant \
                 with a profile.",
            );
            return Ok(Some(ProfileCommands::List));
        },
        1 => Some(ProfileCommands::Edit(EditArgs {
            name: None,
            set_anthropic_key: None,
            set_openai_key: None,
            set_gemini_key: None,
            set_github_token: None,
            set_database_url: None,
            set_external_url: None,
            set_host: None,
            set_port: None,
        })),
        2 => select_profile("Select profile to delete")?
            .map(|name| ProfileCommands::Delete(DeleteArgs { name, yes: false })),
        3 => None,
        other => return Err(anyhow::anyhow!("unexpected menu selection: {other}")),
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
