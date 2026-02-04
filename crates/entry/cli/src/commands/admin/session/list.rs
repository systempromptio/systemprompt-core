//! List available profiles with session status.

use anyhow::Result;
use systemprompt_cloud::{ProfilePath, ProjectContext, SessionKey, SessionStore};
use systemprompt_models::Profile;

use super::types::{ProfileInfo, ProfileListOutput};
use crate::paths::ResolvedPaths;
use crate::shared::CommandResult;
use crate::CliConfig;

pub fn execute(_config: &CliConfig) -> Result<CommandResult<ProfileListOutput>> {
    let project_ctx = ProjectContext::discover();
    let profiles_dir = project_ctx.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(CommandResult::table(ProfileListOutput {
            profiles: Vec::new(),
        })
        .with_title("Available Profiles"));
    }

    let discovered = discover_profiles(&profiles_dir);

    if discovered.is_empty() {
        return Ok(CommandResult::table(ProfileListOutput {
            profiles: Vec::new(),
        })
        .with_title("Available Profiles"));
    }

    let store = ResolvedPaths::discover()
        .sessions_dir()
        .ok()
        .and_then(|dir| SessionStore::load_or_create(&dir).ok());

    let profiles = discovered
        .into_iter()
        .map(|info| build_profile_info(&info, store.as_ref()))
        .collect();

    let output = ProfileListOutput { profiles };

    Ok(CommandResult::table(output)
        .with_title("Available Profiles")
        .with_columns(vec![
            "name".to_string(),
            "routing".to_string(),
            "is_active".to_string(),
            "session_status".to_string(),
        ]))
}

struct DiscoveredProfile {
    name: String,
    tenant_id: Option<String>,
}

fn discover_profiles(dir: &std::path::Path) -> Vec<DiscoveredProfile> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!(path = %dir.display(), error = %e, "Failed to read profiles directory");
            return Vec::new();
        },
    };

    entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let entry_path = e.path();
            let config_path = ProfilePath::Config.resolve(&entry_path);

            if !config_path.exists() {
                return None;
            }

            let name = e.file_name().to_str()?.to_string();
            let profile = load_profile_config(&config_path);

            Some(DiscoveredProfile {
                name,
                tenant_id: profile
                    .as_ref()
                    .and_then(|p| p.cloud.as_ref())
                    .and_then(|c| c.tenant_id.clone()),
            })
        })
        .collect()
}

fn load_profile_config(config_path: &std::path::Path) -> Option<Profile> {
    let content = std::fs::read_to_string(config_path).ok()?;
    Profile::parse(&content, config_path).ok()
}

fn build_profile_info(info: &DiscoveredProfile, store: Option<&SessionStore>) -> ProfileInfo {
    let session_key = SessionKey::from_tenant_id(info.tenant_id.as_deref());

    let is_active = store.is_some_and(|s| {
        s.active_profile_name.as_deref() == Some(info.name.as_str())
            || (s.active_profile_name.is_none()
                && s.active_key.as_ref() == Some(&session_key.as_storage_key()))
    });

    let session_status = store.map_or_else(
        || "unknown".to_string(),
        |s| match s.get_session(&session_key) {
            Some(session) if session.is_expired() => "expired".to_string(),
            Some(session) => {
                let remaining = session.expires_at - chrono::Utc::now();
                let hours = remaining.num_hours();
                let minutes = remaining.num_minutes() % 60;
                format!("{}h {}m remaining", hours, minutes)
            },
            None => "no session".to_string(),
        },
    );

    let routing = info
        .tenant_id
        .as_ref()
        .map_or_else(|| "local".to_string(), |_| "remote".to_string());

    ProfileInfo {
        name: info.name.clone(),
        routing,
        is_active,
        session_status,
    }
}
