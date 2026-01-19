//! Switch to a different profile.

use anyhow::{Context, Result};
use systemprompt_cloud::{get_cloud_paths, CliSession, CloudPath, ProfilePath, ProjectContext};
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

    let session_path = if project_ctx.systemprompt_dir().exists() {
        project_ctx.local_session()
    } else {
        get_cloud_paths()
            .context("Failed to resolve cloud paths")?
            .resolve(CloudPath::CliSession)
    };

    match CliSession::load_from_path(&session_path) {
        Ok(mut session) => {
            if session.profile_name == profile_name {
                if config.is_interactive() {
                    CliService::info(&format!("Already using profile '{}'", profile_name));
                }
                return Ok(());
            }

            let old_tenant_id = get_current_tenant_id(&project_ctx, &session);
            let tenant_changed = old_tenant_id != new_tenant_id;

            session.profile_name = profile_name.to_string();
            session.profile_path = Some(profile_config_path.clone());

            if tenant_changed {
                session.session_token = String::new().into();
                session.session_id = String::new().into();
                session.context_id = String::new().into();
            }

            session.touch();
            session.save_to_path(&session_path)?;

            if tenant_changed && new_tenant_id.is_some() {
                CliService::warning(
                    "Tenant changed. Run 'systemprompt system login' to authenticate with the new \
                     tenant.",
                );
            }
        },
        Err(_) => {
            create_minimal_session(&session_path, profile_name, &profile_config_path)?;
            if new_tenant_id.is_some() {
                CliService::warning(
                    "Run 'systemprompt system login' to authenticate with the tenant.",
                );
            }
        },
    }

    if config.is_interactive() {
        CliService::success(&format!("Switched to profile '{}'", profile_name));
        CliService::key_value("Profile path", &profile_config_path.display().to_string());
        if let Some(tid) = &new_tenant_id {
            CliService::key_value("Tenant", tid);
        }
    }

    Ok(())
}

fn load_profile(path: &std::path::Path) -> Result<Profile> {
    let content = std::fs::read_to_string(path).context("Failed to read profile")?;
    Profile::parse(&content, path).context("Failed to parse profile")
}

fn get_current_tenant_id(project_ctx: &ProjectContext, session: &CliSession) -> Option<String> {
    let profile_path = session.profile_path.as_ref()?;
    if !profile_path.exists() {
        return None;
    }
    let profile = load_profile(profile_path).ok()?;
    profile.cloud.as_ref().and_then(|c| c.tenant_id.clone())
}

fn create_minimal_session(
    session_path: &std::path::Path,
    profile_name: &str,
    profile_config_path: &std::path::Path,
) -> Result<()> {
    use chrono::{Duration, Utc};
    use serde_json::json;

    let now = Utc::now();
    let expires_at = now + Duration::hours(24);

    let session_data = json!({
        "version": 4,
        "profile_name": profile_name,
        "profile_path": profile_config_path,
        "session_token": "",
        "session_id": "",
        "context_id": "",
        "user_id": "system",
        "user_email": "",
        "user_type": "admin",
        "created_at": now,
        "expires_at": expires_at,
        "last_used": now,
    });

    if let Some(dir) = session_path.parent() {
        std::fs::create_dir_all(dir).context("Failed to create session directory")?;
    }

    let content = serde_json::to_string_pretty(&session_data)?;
    std::fs::write(session_path, content).context("Failed to write session file")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(session_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(session_path, perms)?;
    }

    Ok(())
}
