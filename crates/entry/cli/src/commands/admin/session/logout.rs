use anyhow::{Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use systemprompt_cloud::{ProfilePath, SessionKey, SessionStore};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;

#[derive(Debug, Args)]
pub struct LogoutArgs {
    #[arg(long, help = "Profile name to log out (defaults to active session)")]
    pub profile: Option<String>,

    #[arg(short = 'y', long, help = "Skip confirmation prompt")]
    pub yes: bool,

    #[arg(long, help = "Remove all sessions")]
    pub all: bool,
}

pub fn execute(args: &LogoutArgs, config: &CliConfig) -> Result<()> {
    let paths = ResolvedPaths::discover();
    let sessions_dir = paths.sessions_dir()?;
    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    if store.is_empty() {
        CliService::success("No sessions to remove");
        return Ok(());
    }

    if args.all {
        return remove_all_sessions(&store, &sessions_dir, args, config);
    }

    let session_key = resolve_target_key(args, &paths, &store)?;
    let display_name = session_key.to_string();

    if !args.yes && config.is_interactive() {
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Remove session for '{}'?", display_name))
            .default(false)
            .interact()?;

        if !confirmed {
            CliService::info("Cancelled.");
            return Ok(());
        }
    }

    let removed = store.remove_session(&session_key);
    if removed.is_some() {
        store.save(&sessions_dir)?;
        CliService::success(&format!("Session removed for '{}'", display_name));
    } else {
        CliService::warning(&format!("No session found for '{}'", display_name));
    }

    Ok(())
}

fn remove_all_sessions(
    store: &SessionStore,
    sessions_dir: &std::path::Path,
    args: &LogoutArgs,
    config: &CliConfig,
) -> Result<()> {
    let count = store.len();

    if !args.yes {
        if !config.is_interactive() {
            anyhow::bail!("--yes is required in non-interactive mode for --all");
        }

        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Remove all {} session(s)?", count))
            .default(false)
            .interact()?;

        if !confirmed {
            CliService::info("Cancelled.");
            return Ok(());
        }
    }

    let new_store = SessionStore::new();
    new_store.save(sessions_dir)?;
    CliService::success(&format!("Removed {} session(s)", count));
    Ok(())
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

        let content = std::fs::read_to_string(&profile_config_path)
            .with_context(|| format!("Failed to read profile '{}'", profile_name))?;
        let profile = Profile::parse(&content, &profile_config_path)
            .with_context(|| format!("Failed to parse profile '{}'", profile_name))?;

        let tenant_id = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_deref());
        return Ok(SessionKey::from_tenant_id(tenant_id));
    }

    store.active_session_key().ok_or_else(|| {
        anyhow::anyhow!(
            "No active session. Use --profile <name> to specify which session to remove."
        )
    })
}
