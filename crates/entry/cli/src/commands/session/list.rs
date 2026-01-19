//! List available profiles.

use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_core_logging::CliService;
use systemprompt_models::Profile;

use crate::cli_settings::CliConfig;

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

    if config.is_interactive() {
        CliService::section("Available Profiles");
    }

    for profile_info in profiles {
        print_profile_info(&profile_info, config);
    }
}

struct ProfileInfo {
    name: String,
    display_name: Option<String>,
    tenant_id: Option<String>,
}

fn discover_profiles(dir: &std::path::Path) -> Vec<ProfileInfo> {
    std::fs::read_dir(dir)
        .map(|entries| {
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
        })
        .unwrap_or_default()
}

fn load_profile_info(config_path: &std::path::Path) -> Option<Profile> {
    let content = std::fs::read_to_string(config_path).ok()?;
    Profile::parse(&content, config_path).ok()
}

fn print_profile_info(info: &ProfileInfo, config: &CliConfig) {
    if config.is_interactive() {
        let display = info
            .display_name
            .as_ref()
            .map_or_else(String::new, |d| format!(" ({})", d));

        let routing = info
            .tenant_id
            .as_ref()
            .map_or_else(|| "Local".to_string(), |tid| format!("Remote -> {}", tid));

        CliService::output(&format!("  {}{} [{}]", info.name, display, routing));
    } else {
        CliService::output(&info.name);
    }
}
