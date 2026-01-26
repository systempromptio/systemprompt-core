use anyhow::Result;
use systemprompt_cloud::{SessionKey, SessionStore, TenantStore, LOCAL_SESSION_KEY};
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::paths::ResolvedPaths;

#[allow(clippy::unnecessary_wraps)]
pub fn execute(config: &CliConfig) -> Result<()> {
    let paths = ResolvedPaths::discover();

    if config.is_interactive() {
        CliService::section("Sessions");
    }

    display_all_sessions(&paths, config.profile_override.as_deref());

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

fn display_all_sessions(paths: &ResolvedPaths, profile_filter: Option<&str>) {
    let Ok(sessions_dir) = paths.sessions_dir() else {
        CliService::warning("No sessions found");
        return;
    };

    let Ok(store) = SessionStore::load_or_create(&sessions_dir) else {
        CliService::warning("No sessions found");
        return;
    };

    let active_key = store.active_key.clone();

    if store.is_empty() && active_key.is_none() {
        CliService::warning("No sessions found");
        return;
    }

    let mut displayed_active = false;

    for (key, session) in store.all_sessions() {
        if let Some(filter) = profile_filter {
            if session.profile_name.as_str() != filter {
                continue;
            }
        }

        let is_active = active_key.as_ref() == Some(key);
        if is_active {
            displayed_active = true;
        }
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

        let context_age = chrono::Utc::now() - session.last_used;
        if context_age.num_hours() > 24 {
            CliService::warning(&format!(
                "    Context may be stale (last used {}h ago). Re-login with --force-new if \
                 commands fail.",
                context_age.num_hours()
            ));
        }
    }

    if !displayed_active {
        if let Some(ref key_str) = active_key {
            let display_key = if key_str == LOCAL_SESSION_KEY {
                "local".to_string()
            } else {
                key_str
                    .strip_prefix("tenant_")
                    .map_or_else(|| key_str.clone(), String::from)
            };
            CliService::output(&format!("\n  {} (active) - no session", display_key));
            CliService::warning(
                "    Run 'systemprompt admin session login' to create a session for this profile.",
            );
        }
    }
}

fn display_routing_info(paths: &ResolvedPaths) -> Option<(String, &'static str)> {
    let sessions_dir = paths.sessions_dir().ok()?;
    let store = SessionStore::load_or_create(&sessions_dir).ok()?;
    let active_key = store.active_session_key()?;

    let session = store.sessions.get(&active_key.as_storage_key());

    let profile_name = session.map_or_else(
        || "unknown".to_string(),
        |s| s.profile_name.as_str().to_string(),
    );

    CliService::key_value("Profile name", &profile_name);

    match &active_key {
        SessionKey::Local => {
            CliService::key_value("Target", "Local");
            None
        },
        SessionKey::Tenant(tenant_id) => {
            CliService::key_value("Tenant ID", tenant_id.as_str());
            resolve_remote_target(paths, tenant_id.as_str())
        },
    }
}

fn resolve_remote_target(paths: &ResolvedPaths, tenant_id: &str) -> Option<(String, &'static str)> {
    let tenants_path = paths.tenants_path().ok()?;

    let store = TenantStore::load_from_path(&tenants_path).ok()?;
    let tenant = store.find_tenant(tenant_id)?;

    tenant.hostname.as_ref().map(|h| (h.clone(), "Remote"))
}
