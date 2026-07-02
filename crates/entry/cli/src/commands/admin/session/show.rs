use systemprompt_cloud::{CliSession, LOCAL_SESSION_KEY, SessionKey, SessionStore, TenantStore};

use super::types::{RoutingInfo, SessionInfo, SessionShowOutput};
use crate::CliConfig;
use crate::paths::ResolvedPaths;
use crate::shared::CommandOutput;

pub(super) fn execute(_config: &CliConfig) -> CommandOutput {
    let paths = ResolvedPaths::discover();

    let sessions = collect_sessions(&paths);
    let routing = collect_routing_info(&paths);

    let output = SessionShowOutput { sessions, routing };

    CommandOutput::card_value("Session Info", &output)
}

fn collect_sessions(paths: &ResolvedPaths) -> Vec<SessionInfo> {
    let sessions_dir = paths.sessions_dir();

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
        results.push(session_info(key, session, is_active));
    }

    if !displayed_active && (active_key.is_some() || active_profile.is_some()) {
        results.push(missing_active_session(
            active_key.as_deref(),
            active_profile.as_deref(),
        ));
    }

    results
}

fn session_info(key: &str, session: &CliSession, is_active: bool) -> SessionInfo {
    let display_key = if key == LOCAL_SESSION_KEY {
        "local".to_owned()
    } else {
        key.strip_prefix("tenant_")
            .map_or_else(|| key.to_owned(), String::from)
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
                "Context may be stale (last used {}h ago). Re-login with --force-new if commands \
                 fail.",
                context_age.num_hours()
            ))
        } else {
            None
        }
    };

    SessionInfo {
        key: display_key,
        profile_name: session.profile_name.as_str().to_owned(),
        user_email: session.user_email.as_str().to_owned(),
        session_id: Some(session.session_id.clone()),
        context_id: Some(session.context_id.clone()),
        is_active,
        is_expired: session.is_expired(),
        expires_in,
        stale_warning,
    }
}

fn missing_active_session(active_key: Option<&str>, active_profile: Option<&str>) -> SessionInfo {
    let display_name = active_profile.unwrap_or_else(|| {
        active_key.map_or("unknown", |k| {
            if k == LOCAL_SESSION_KEY {
                "local"
            } else {
                k.strip_prefix("tenant_").unwrap_or(k)
            }
        })
    });

    SessionInfo {
        key: display_name.to_owned(),
        profile_name: display_name.to_owned(),
        user_email: String::new(),
        session_id: None,
        context_id: None,
        is_active: true,
        is_expired: false,
        expires_in: None,
        stale_warning: Some(
            "No session. Run 'systemprompt admin session login' to create a session.".to_owned(),
        ),
    }
}

fn collect_routing_info(paths: &ResolvedPaths) -> Option<RoutingInfo> {
    let sessions_dir = paths.sessions_dir();
    let store = SessionStore::load_or_create(&sessions_dir).ok()?;
    let active_key = store.active_session_key()?;

    let session = store.sessions.get(&active_key.as_storage_key());

    let profile_name = session
        .map(|s| s.profile_name.as_str().to_owned())
        .or_else(|| store.active_profile_name.clone())
        .unwrap_or_else(|| "unknown".to_owned());

    match &active_key {
        SessionKey::Local => Some(RoutingInfo {
            profile_name,
            target: "Local".to_owned(),
            tenant: None,
            hostname: None,
        }),
        SessionKey::Tenant(tenant_id) => {
            let hostname = resolve_remote_hostname(paths, tenant_id.as_str());
            Some(RoutingInfo {
                profile_name,
                target: if hostname.is_some() {
                    "Remote".to_owned()
                } else {
                    "Tenant".to_owned()
                },
                tenant: Some(tenant_id.as_str().to_owned()),
                hostname,
            })
        },
    }
}

fn resolve_remote_hostname(paths: &ResolvedPaths, tenant: &str) -> Option<String> {
    let tenants_path = paths.tenants_path();
    let store = TenantStore::load_from_path(&tenants_path).ok()?;
    let tenant = store.find_tenant(&systemprompt_identifiers::TenantId::new(tenant))?;
    tenant.hostname.clone()
}
