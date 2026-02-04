use anyhow::Result;
use systemprompt_cloud::{SessionKey, SessionStore, TenantStore, LOCAL_SESSION_KEY};

use super::types::{RoutingInfo, SessionInfo, SessionShowOutput};
use crate::paths::ResolvedPaths;
use crate::shared::CommandResult;
use crate::CliConfig;

#[allow(clippy::unnecessary_wraps)]
pub fn execute(_config: &CliConfig) -> Result<CommandResult<SessionShowOutput>> {
    let paths = ResolvedPaths::discover();

    let sessions = collect_sessions(&paths);
    let routing = collect_routing_info(&paths);

    let output = SessionShowOutput { sessions, routing };

    Ok(CommandResult::card(output).with_title("Session Info"))
}

fn collect_sessions(paths: &ResolvedPaths) -> Vec<SessionInfo> {
    let Ok(sessions_dir) = paths.sessions_dir() else {
        return Vec::new();
    };

    let Ok(store) = SessionStore::load_or_create(&sessions_dir) else {
        return Vec::new();
    };

    let active_key = store.active_key.clone();
    let active_profile = store.active_profile_name.clone();

    let mut results = Vec::new();
    let mut displayed_active = false;

    for (key, session) in store.all_sessions() {
        let is_active = active_key.as_ref() == Some(key);
        if is_active {
            displayed_active = true;
        }

        let display_key = if key == LOCAL_SESSION_KEY {
            "local".to_string()
        } else {
            key.strip_prefix("tenant_")
                .map_or_else(|| key.clone(), String::from)
        };

        let expires_in = if session.is_expired() {
            None
        } else {
            let remaining = session.expires_at - chrono::Utc::now();
            let hours = remaining.num_hours();
            let minutes = remaining.num_minutes() % 60;
            Some(format!("{}h {}m", hours, minutes))
        };

        let stale_warning = {
            let context_age = chrono::Utc::now() - session.last_used;
            if context_age.num_hours() > 24 {
                Some(format!(
                    "Context may be stale (last used {}h ago). Re-login with --force-new if \
                     commands fail.",
                    context_age.num_hours()
                ))
            } else {
                None
            }
        };

        results.push(SessionInfo {
            key: display_key,
            profile_name: session.profile_name.as_str().to_string(),
            user_email: session.user_email.as_str().to_string(),
            session_id: session.session_id.as_str().to_string(),
            context_id: session.context_id.as_str().to_string(),
            is_active,
            is_expired: session.is_expired(),
            expires_in,
            stale_warning,
        });
    }

    if !displayed_active && (active_key.is_some() || active_profile.is_some()) {
        let display_name = active_profile.as_deref().unwrap_or_else(|| {
            active_key.as_deref().map_or("unknown", |k| {
                if k == LOCAL_SESSION_KEY {
                    "local"
                } else {
                    k.strip_prefix("tenant_").unwrap_or(k)
                }
            })
        });

        results.push(SessionInfo {
            key: display_name.to_string(),
            profile_name: display_name.to_string(),
            user_email: String::new(),
            session_id: String::new(),
            context_id: String::new(),
            is_active: true,
            is_expired: false,
            expires_in: None,
            stale_warning: Some(
                "No session. Run 'systemprompt admin session login' to create a session."
                    .to_string(),
            ),
        });
    }

    results
}

fn collect_routing_info(paths: &ResolvedPaths) -> Option<RoutingInfo> {
    let sessions_dir = paths.sessions_dir().ok()?;
    let store = SessionStore::load_or_create(&sessions_dir).ok()?;
    let active_key = store.active_session_key()?;

    let session = store.sessions.get(&active_key.as_storage_key());

    let profile_name = session
        .map(|s| s.profile_name.as_str().to_string())
        .or_else(|| store.active_profile_name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    match &active_key {
        SessionKey::Local => Some(RoutingInfo {
            profile_name,
            target: "Local".to_string(),
            tenant_id: None,
            hostname: None,
        }),
        SessionKey::Tenant(tenant_id) => {
            let hostname = resolve_remote_hostname(paths, tenant_id.as_str());
            Some(RoutingInfo {
                profile_name,
                target: if hostname.is_some() {
                    "Remote".to_string()
                } else {
                    "Tenant".to_string()
                },
                tenant_id: Some(tenant_id.as_str().to_string()),
                hostname,
            })
        },
    }
}

fn resolve_remote_hostname(paths: &ResolvedPaths, tenant_id: &str) -> Option<String> {
    let tenants_path = paths.tenants_path().ok()?;
    let store = TenantStore::load_from_path(&tenants_path).ok()?;
    let tenant = store.find_tenant(tenant_id)?;
    tenant.hostname.clone()
}
