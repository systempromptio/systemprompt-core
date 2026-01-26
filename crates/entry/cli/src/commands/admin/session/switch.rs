#![allow(clippy::single_match_else)]

use anyhow::{Context, Result};
use systemprompt_cloud::{ProfilePath, SessionKey, SessionStore};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;

pub fn execute(profile_name: &str, config: &CliConfig) -> Result<()> {
    let paths = ResolvedPaths::discover();
    let profiles_dir = paths.profiles_dir();

    let target_dir = profiles_dir.join(profile_name);
    let profile_config_path = ProfilePath::Config.resolve(&target_dir);

    if !profile_config_path.exists() {
        anyhow::bail!(
            "Profile '{}' not found.\n\nAvailable profiles can be listed with: systemprompt admin \
             session list",
            profile_name
        );
    }

    let new_profile = load_profile(&profile_config_path)?;
    let new_tenant_id = new_profile.cloud.as_ref().and_then(|c| c.tenant_id.clone());
    let session_key = SessionKey::from_tenant_id(new_tenant_id.as_deref());

    let sessions_dir = paths.sessions_dir()?;
    let mut store = SessionStore::load_or_create(&sessions_dir)?;

    store.set_active_with_profile(&session_key, profile_name);
    store.save(&sessions_dir)?;

    let has_session = store.get_valid_session(&session_key).is_some();
    if !has_session {
        CliService::warning(
            "No active session for this profile. Run 'systemprompt admin session login' to \
             authenticate.",
        );
    }

    CliService::success(&format!("Switched to profile '{}'", profile_name));

    if config.is_interactive() {
        CliService::key_value("Profile path", &profile_config_path.display().to_string());
        CliService::key_value("Session key", &session_key.as_storage_key());
        if let Some(tid) = &new_tenant_id {
            CliService::key_value("Tenant", tid);
        }
        if has_session {
            CliService::info("Session available for this tenant");
        }
    }

    Ok(())
}

fn load_profile(path: &std::path::Path) -> Result<Profile> {
    let content = std::fs::read_to_string(path).context("Failed to read profile")?;
    Profile::parse(&content, path).context("Failed to parse profile")
}
