//! Show current session and routing info.

use anyhow::{Context, Result};
use systemprompt_cloud::{get_cloud_paths, CliSession, CloudPath, ProjectContext, TenantStore};
use systemprompt_core_logging::CliService;
use systemprompt_models::ProfileBootstrap;

use crate::cli_settings::CliConfig;

pub fn execute(config: &CliConfig) -> Result<()> {
    let project_ctx = ProjectContext::discover();
    let session_path = if project_ctx.systemprompt_dir().exists() {
        project_ctx.local_session()
    } else {
        get_cloud_paths()
            .context("Failed to resolve cloud paths")?
            .resolve(CloudPath::CliSession)
    };

    if config.is_interactive() {
        CliService::section("Session Info");
    }

    display_session_info(&session_path);

    CliService::output("");

    if config.is_interactive() {
        CliService::section("Routing Info");
    }

    let execution_target = display_routing_info(&project_ctx);

    if let Some((hostname, target_type)) = execution_target {
        CliService::key_value("Target", target_type);
        CliService::key_value("Hostname", &hostname);
        CliService::info("Commands will be forwarded to the remote tenant");
    }

    Ok(())
}

fn display_session_info(session_path: &std::path::Path) {
    let session = CliSession::load_from_path(session_path).ok();

    let Some(session) = session else {
        CliService::warning("No active session found");
        return;
    };

    CliService::key_value("Profile", &session.profile_name);
    CliService::key_value("User", &session.user_email);
    CliService::key_value("Session ID", session.session_id.as_str());
    CliService::key_value("Context ID", session.context_id.as_str());

    if session.is_expired() {
        CliService::warning("Session has expired");
    } else {
        let expires_in = session.expires_at - chrono::Utc::now();
        let hours = expires_in.num_hours();
        let minutes = expires_in.num_minutes() % 60;
        CliService::key_value("Expires in", &format!("{}h {}m", hours, minutes));
    }
}

fn display_routing_info(project_ctx: &ProjectContext) -> Option<(String, &'static str)> {
    let profile = ProfileBootstrap::get().ok();

    let Some(p) = profile else {
        CliService::warning("No profile loaded");
        return None;
    };

    CliService::key_value("Profile name", &p.name);

    let tenant_id = p.cloud.as_ref().and_then(|c| c.tenant_id.as_ref());

    let Some(tenant_id) = tenant_id else {
        CliService::key_value("Target", "Local");
        return None;
    };

    CliService::key_value("Tenant ID", tenant_id);
    resolve_remote_target(project_ctx, tenant_id)
}

fn resolve_remote_target(
    project_ctx: &ProjectContext,
    tenant_id: &str,
) -> Option<(String, &'static str)> {
    let tenants_path = if project_ctx.systemprompt_dir().exists() {
        project_ctx.local_tenants()
    } else {
        get_cloud_paths().ok()?.resolve(CloudPath::Tenants)
    };

    let store = TenantStore::load_from_path(&tenants_path).ok()?;
    let tenant = store.find_tenant(tenant_id)?;

    tenant.hostname.as_ref().map(|h| (h.clone(), "Remote"))
}
