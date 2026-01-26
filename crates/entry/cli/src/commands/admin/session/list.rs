//! List available profiles with session status.

use systemprompt_cloud::{ProfilePath, ProjectContext, SessionKey, SessionStore};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;

pub fn execute(config: &CliConfig) {
    let project_ctx = ProjectContext::discover();
    let profiles_dir = project_ctx.profiles_dir();

    if !profiles_dir.exists() {
        CliService::warning("No profiles directory found");
        CliService::info("Create a profile with: systemprompt cloud profile create <name>");
        return;
    }

    let profiles = discover_profiles(&profiles_dir);

    if profiles.is_empty() {
        CliService::warning("No profiles found");
        CliService::info("Create a profile with: systemprompt cloud profile create <name>");
        return;
    }

    let store = ResolvedPaths::discover()
        .sessions_dir()
        .ok()
        .and_then(|dir| SessionStore::load_or_create(&dir).ok());

    if config.is_interactive() {
        CliService::section("Available Profiles");
    }

    for profile_info in profiles {
        print_profile_info(&profile_info, store.as_ref(), config);
    }
}

struct ProfileInfo {
    name: String,
    tenant_id: Option<String>,
}

fn discover_profiles(dir: &std::path::Path) -> Vec<ProfileInfo> {
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
            let profile = load_profile_info(&config_path);

            Some(ProfileInfo {
                name,
                tenant_id: profile
                    .as_ref()
                    .and_then(|p| p.cloud.as_ref())
                    .and_then(|c| c.tenant_id.clone()),
            })
        })
        .collect()
}

fn load_profile_info(config_path: &std::path::Path) -> Option<Profile> {
    let content = std::fs::read_to_string(config_path).ok()?;
    Profile::parse(&content, config_path).ok()
}

fn print_profile_info(info: &ProfileInfo, store: Option<&SessionStore>, config: &CliConfig) {
    if config.is_interactive() {
        let session_key = SessionKey::from_tenant_id(info.tenant_id.as_deref());

        let is_active = store.is_some_and(|s| {
            s.active_profile_name.as_deref() == Some(info.name.as_str())
                || (s.active_profile_name.is_none()
                    && s.active_key.as_ref() == Some(&session_key.as_storage_key()))
        });

        let active_marker = if is_active { " (active)" } else { "" };

        let session_info = store.map_or_else(String::new, |s| match s.get_session(&session_key) {
            Some(session) if session.is_expired() => "  [expired]".to_string(),
            Some(session) => {
                let remaining = session.expires_at - chrono::Utc::now();
                let hours = remaining.num_hours();
                let minutes = remaining.num_minutes() % 60;
                format!("  [session: {}h {}m remaining]", hours, minutes)
            },
            None => "  [no session]".to_string(),
        });

        let routing = info
            .tenant_id
            .as_ref()
            .map_or_else(|| "local".to_string(), |_| "remote".to_string());

        CliService::output(&format!(
            "  {:<16} {:<8}{}{}",
            info.name, routing, active_marker, session_info
        ));
    } else {
        CliService::output(&info.name);
    }
}
