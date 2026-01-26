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
    display_name: Option<String>,
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
                display_name: profile.as_ref().map(|p| p.display_name.clone()),
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
        let display = info
            .display_name
            .as_ref()
            .map_or_else(String::new, |d| format!(" ({})", d));

        let routing = info
            .tenant_id
            .as_ref()
            .map_or_else(|| "Local".to_string(), |tid| format!("Remote -> {}", tid));

        let session_key = SessionKey::from_tenant_id(info.tenant_id.as_deref());
        let (active_marker, session_status) = store.map_or(("", ""), |s| {
            let is_active = s.active_key.as_ref() == Some(&session_key.as_storage_key());
            let marker = if is_active { " *" } else { "" };
            let status = match s.get_session(&session_key) {
                Some(session) if session.is_expired() => " [expired]",
                Some(_) => " [session]",
                None => "",
            };
            (marker, status)
        });

        CliService::output(&format!(
            "  {}{}{} [{}]{}",
            info.name, display, active_marker, routing, session_status
        ));
    } else {
        CliService::output(&info.name);
    }
}
