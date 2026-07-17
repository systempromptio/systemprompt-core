//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_cloud::{ProfilePath, SessionKey, SessionStore};
use systemprompt_loader::ProfileLoader;

use super::types::LogoutOutput;
use crate::CliConfig;
use crate::interactive::Prompter;
use crate::paths::ResolvedPaths;
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct LogoutArgs {
    #[arg(long, help = "Profile name to log out (defaults to active session)")]
    pub profile: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,

    #[arg(long, help = "Remove all sessions")]
    pub all: bool,
}

pub(super) fn execute(
    args: &LogoutArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let paths = ResolvedPaths::discover();
    let sessions_dir = paths.sessions_dir();
    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    if store.is_empty() {
        return Ok(CommandOutput::card_value(
            "Logout",
            &LogoutOutput {
                action: "none".to_owned(),
                target: "all".to_owned(),
                message: "No sessions to remove".to_owned(),
            },
        ));
    }

    if args.all {
        return remove_all_sessions(&store, &sessions_dir, args, prompter, config);
    }

    let session_key = resolve_target_key(args, &paths, &store)?;
    let display_name = session_key.to_string();

    if !args.yes && config.is_interactive() {
        let confirmed =
            prompter.confirm(&format!("Remove session for '{}'?", display_name), false)?;

        if !confirmed {
            return Ok(CommandOutput::card_value(
                "Logout",
                &LogoutOutput {
                    action: "cancelled".to_owned(),
                    target: display_name,
                    message: "Operation cancelled".to_owned(),
                },
            ));
        }
    }

    let removed = store.remove_session(&session_key);
    if removed.is_some() {
        store.save(&sessions_dir)?;
        Ok(CommandOutput::card_value(
            "Logout",
            &LogoutOutput {
                action: "removed".to_owned(),
                target: display_name.clone(),
                message: format!("Session removed for '{}'", display_name),
            },
        ))
    } else {
        Ok(CommandOutput::card_value(
            "Logout",
            &LogoutOutput {
                action: "not_found".to_owned(),
                target: display_name.clone(),
                message: format!("No session found for '{}'", display_name),
            },
        ))
    }
}

fn remove_all_sessions(
    store: &SessionStore,
    sessions_dir: &std::path::Path,
    args: &LogoutArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let count = store.len();

    if !args.yes {
        if !config.is_interactive() {
            anyhow::bail!("--yes is required in non-interactive mode for --all");
        }

        let confirmed = prompter.confirm(&format!("Remove all {} session(s)?", count), false)?;

        if !confirmed {
            return Ok(CommandOutput::card_value(
                "Logout",
                &LogoutOutput {
                    action: "cancelled".to_owned(),
                    target: "all".to_owned(),
                    message: "Operation cancelled".to_owned(),
                },
            ));
        }
    }

    let new_store = SessionStore::new();
    new_store.save(sessions_dir)?;

    Ok(CommandOutput::card_value(
        "Logout",
        &LogoutOutput {
            action: "removed_all".to_owned(),
            target: "all".to_owned(),
            message: format!("Removed {} session(s)", count),
        },
    ))
}

fn resolve_target_key(
    args: &LogoutArgs,
    paths: &ResolvedPaths,
    store: &SessionStore,
) -> Result<SessionKey> {
    if let Some(ref profile_name) = args.profile {
        let target_dir = paths.profiles_dir().join(profile_name);
        let profile_config_path = ProfilePath::Config.resolve(&target_dir);

        if !profile_config_path.exists() {
            anyhow::bail!(
                "Profile '{}' not found.\n\nAvailable profiles can be listed with: systemprompt \
                 admin session list",
                profile_name
            );
        }

        let profile = ProfileLoader::load_from_path(&profile_config_path)
            .with_context(|| format!("Failed to load profile '{}'", profile_name))?;

        let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref());
        return Ok(SessionKey::from_tenant_id(tenant_id));
    }

    store.active_session_key().ok_or_else(|| {
        anyhow::anyhow!(
            "No active session. Use --profile <name> to specify which session to remove."
        )
    })
}
