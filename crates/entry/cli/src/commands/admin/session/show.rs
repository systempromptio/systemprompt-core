use anyhow::Result;
use systemprompt_cloud::{SessionStore, TenantStore, LOCAL_SESSION_KEY};
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;

#[allow(clippy::unnecessary_wraps)]
pub fn execute(config: &CliConfig) -> Result<()> {
    let paths = ResolvedPaths::discover();

    if config.is_interactive() {
        CliService::section("Sessions");
    }

    display_all_sessions(&paths);

    CliService::output("");

    if config.is_interactive() {
        CliService::section("Routing Info");
    }

    if let Some((hostname, target_type)) = display_routing_info(&paths) {
        CliService::key_value("Target", target_type);
        CliService::key_value("Hostname", &hostname);
        CliService::info("Commands will be forwarded to the remote tenant");
    }

    Ok(())
}

fn display_all_sessions(paths: &ResolvedPaths) {
    let Ok(sessions_dir) = paths.sessions_dir() else {
        CliService::warning("No sessions found");
        return;
    };

    let Ok(store) = SessionStore::load_or_create(&sessions_dir) else {
        CliService::warning("No sessions found");
        return;
    };

    if store.is_empty() {
        CliService::warning("No sessions found");
        return;
    }

    let active_key = store.active_key.clone();

    for (key, session) in store.all_sessions() {
        let is_active = active_key.as_ref() == Some(key);
        let status_marker = if is_active { " (active)" } else { "" };
        let expired_marker = if session.is_expired() {
            " [expired]"
        } else {
            ""
        };

        let display_key = if key == LOCAL_SESSION_KEY {
            "local".to_string()
        } else {
            key.strip_prefix("tenant_")
                .map_or_else(|| key.clone(), String::from)
        };

        CliService::output(&format!(
            "\n  {}{}{}",
            display_key, status_marker, expired_marker
        ));
        CliService::key_value("    Profile", session.profile_name.as_str());
        CliService::key_value("    User", session.user_email.as_str());
        CliService::key_value("    Session ID", session.session_id.as_str());
        CliService::key_value("    Context ID", session.context_id.as_str());

        if session.is_expired() {
            CliService::warning("    Session has expired");
        } else {
            let expires_in = session.expires_at - chrono::Utc::now();
            let hours = expires_in.num_hours();
            let minutes = expires_in.num_minutes() % 60;
            CliService::key_value("    Expires in", &format!("{}h {}m", hours, minutes));
        }
    }
}

fn display_routing_info(paths: &ResolvedPaths) -> Option<(String, &'static str)> {
    let Ok(profile) = ProfileBootstrap::get() else {
        CliService::warning("No profile loaded");
        return None;
    };

    CliService::key_value("Profile name", &profile.name);

    let Some(tenant_id) = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref()) else {
        CliService::key_value("Target", "Local");
        return None;
    };

    CliService::key_value("Tenant ID", tenant_id);
    resolve_remote_target(paths, tenant_id)
}

fn resolve_remote_target(paths: &ResolvedPaths, tenant_id: &str) -> Option<(String, &'static str)> {
    let tenants_path = paths.tenants_path().ok()?;

    let store = TenantStore::load_from_path(&tenants_path).ok()?;
    let tenant = store.find_tenant(tenant_id)?;

    tenant.hostname.as_ref().map(|h| (h.clone(), "Remote"))
}
