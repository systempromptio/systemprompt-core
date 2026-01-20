#![allow(clippy::single_match_else)]

use anyhow::{Context, Result};
use systemprompt_cloud::{
    get_cloud_paths, CloudPath, ProfilePath, ProjectContext, SessionKey, SessionStore,
};
use systemprompt_core_logging::CliService;
use systemprompt_models::Profile;

use crate::cli_settings::CliConfig;

pub fn execute(profile_name: &str, config: &CliConfig) -> Result<()> {
    let project_ctx = ProjectContext::discover();
    let profiles_dir = project_ctx.profiles_dir();

    let target_dir = profiles_dir.join(profile_name);
    let profile_config_path = ProfilePath::Config.resolve(&target_dir);

    if !profile_config_path.exists() {
        anyhow::bail!(
            "Profile '{}' not found.\n\nAvailable profiles can be listed with: systemprompt \
             session list",
            profile_name
        );
    }

    let new_profile = load_profile(&profile_config_path)?;
    let new_tenant_id = new_profile.cloud.as_ref().and_then(|c| c.tenant_id.clone());
    let session_key = SessionKey::from_tenant_id(new_tenant_id.as_deref());

    let (sessions_dir, legacy_path) = resolve_session_paths(&project_ctx)?;
    let mut store = SessionStore::load_or_create(&sessions_dir, legacy_path.as_deref())?;

    store.set_active(&session_key);
    store.save(&sessions_dir)?;

    let has_session = store.get_valid_session(&session_key).is_some();
    if !has_session && new_tenant_id.is_some() {
        CliService::warning(
            "No session for this tenant. Run 'systemprompt infra system login' to authenticate.",
        );
    }

    if config.is_interactive() {
        CliService::success(&format!("Switched to profile '{}'", profile_name));
        CliService::key_value("Profile path", &profile_config_path.display().to_string());
        if let Some(tid) = &new_tenant_id {
            CliService::key_value("Tenant", tid);
        }
        if has_session {
            CliService::info("Session available for this tenant");
        }
    }

    Ok(())
}

fn resolve_session_paths(
    project_ctx: &ProjectContext,
) -> Result<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    if project_ctx.systemprompt_dir().exists() {
        Ok((
            project_ctx.sessions_dir(),
            Some(project_ctx.local_session()),
        ))
    } else {
        let cloud_paths = get_cloud_paths().context("Failed to resolve cloud paths")?;
        Ok((
            cloud_paths.resolve(CloudPath::SessionsDir),
            Some(cloud_paths.resolve(CloudPath::CliSession)),
        ))
    }
}

fn load_profile(path: &std::path::Path) -> Result<Profile> {
    let content = std::fs::read_to_string(path).context("Failed to read profile")?;
    Profile::parse(&content, path).context("Failed to parse profile")
}
