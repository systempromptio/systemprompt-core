//! List available profiles with session status.

use systemprompt_cloud::{ProfilePath, ProjectContext, SessionKey, SessionStore};
use systemprompt_loader::ProfileLoader;
use systemprompt_models::Profile;

use super::types::{ProfileInfo, ProfileListOutput};
use crate::CliConfig;
use crate::paths::ResolvedPaths;
use crate::shared::CommandOutput;

pub(super) fn execute(_config: &CliConfig) -> CommandOutput {
    let project_ctx = ProjectContext::discover();
    let profiles_dir = project_ctx.profiles_dir();

    if !profiles_dir.exists() {
        let output = ProfileListOutput {
            profiles: Vec::new(),
        };
        return CommandOutput::table_of(
            vec!["name", "routing", "is_active", "session_status"],
            &output.profiles,
        )
        .with_title("Available Profiles");
    }

    let discovered = discover_profiles(&profiles_dir);

    if discovered.is_empty() {
        let output = ProfileListOutput {
            profiles: Vec::new(),
        };
        return CommandOutput::table_of(
            vec!["name", "routing", "is_active", "session_status"],
            &output.profiles,
        )
        .with_title("Available Profiles");
    }

    let store = {
        let dir = ResolvedPaths::discover().sessions_dir();
        SessionStore::load_or_create(&dir).ok()
    };

    let profiles = discovered
        .into_iter()
        .map(|info| build_profile_info(&info, store.as_ref()))
        .collect();

    let output = ProfileListOutput { profiles };

    CommandOutput::table_of(
        vec!["name", "routing", "is_active", "session_status"],
        &output.profiles,
    )
    .with_title("Available Profiles")
}

struct DiscoveredProfile {
    name: String,
    tenant: Option<systemprompt_identifiers::TenantId>,
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

            let name = e.file_name().to_str()?.to_owned();
            let profile = load_profile_config(&config_path);

            Some(DiscoveredProfile {
                name,
                tenant: profile
                    .as_ref()
                    .and_then(|p| p.cloud.as_ref())
                    .and_then(|c| c.tenant_id.clone()),
            })
        })
        .collect()
}

fn load_profile_config(config_path: &std::path::Path) -> Option<Profile> {
    ProfileLoader::load_from_path(config_path).ok()
}

fn build_profile_info(info: &DiscoveredProfile, store: Option<&SessionStore>) -> ProfileInfo {
    let session_key = SessionKey::from_tenant_id(info.tenant.as_ref());

    let is_active = store.is_some_and(|s| {
        s.active_profile_name.as_deref() == Some(info.name.as_str())
            || (s.active_profile_name.is_none()
                && s.active_key.as_ref() == Some(&session_key.as_storage_key()))
    });

    let session_status = store.map_or_else(
        || "unknown".to_owned(),
        |s| match s.get_session(&session_key) {
            Some(session) if session.is_expired() => "expired".to_owned(),
            Some(session) => {
                let remaining = session.expires_at - chrono::Utc::now();
                let hours = remaining.num_hours();
                let minutes = remaining.num_minutes() % 60;
                format!("{}h {}m remaining", hours, minutes)
            },
            None => "no session".to_owned(),
        },
    );

    let routing = info
        .tenant
        .as_ref()
        .map_or_else(|| "local".to_owned(), |_| "remote".to_owned());

    ProfileInfo {
        name: info.name.clone(),
        display_name: None,
        database_url: None,
        tenant_id: info.tenant.clone(),
        validation_mode: None,
        credentials_path: None,
        routing: Some(routing),
        is_active: Some(is_active),
        session_status: Some(session_status),
    }
}
