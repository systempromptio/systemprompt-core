pub mod remote;

use anyhow::{Context, Result};
use systemprompt_cloud::{
    get_cloud_paths, CloudPath, ProjectContext, SessionKey, SessionStore, StoredTenant, TenantStore,
};
use systemprompt_identifiers::TenantId;
use systemprompt_models::ProfileBootstrap;

pub enum ExecutionTarget {
    Local,
    Remote {
        hostname: String,
        token: String,
        context_id: String,
    },
}

pub fn determine_execution_target() -> Result<ExecutionTarget> {
    let Ok(profile) = ProfileBootstrap::get() else {
        return Ok(ExecutionTarget::Local);
    };

    let Some(tenant_id) = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref()) else {
        return Ok(ExecutionTarget::Local);
    };

    let tenant = resolve_tenant(tenant_id)?;
    let hostname = tenant
        .hostname
        .as_ref()
        .context("Tenant has no hostname configured")?
        .clone();

    let session_key = SessionKey::Tenant(TenantId::new(tenant_id.clone()));
    let session = load_session_for_key(&session_key)?;

    Ok(ExecutionTarget::Remote {
        hostname,
        token: session.session_token.to_string(),
        context_id: session.context_id.to_string(),
    })
}

fn resolve_tenant(tenant_id: &str) -> Result<StoredTenant> {
    let project_ctx = ProjectContext::discover();

    let tenants_path = if project_ctx.systemprompt_dir().exists() {
        project_ctx.local_tenants()
    } else {
        get_cloud_paths()
            .context("Failed to resolve cloud paths")?
            .resolve(CloudPath::Tenants)
    };

    let store = TenantStore::load_from_path(&tenants_path)
        .context("Failed to load tenants. Run 'systemprompt cloud tenant list' to sync.")?;

    store
        .find_tenant(tenant_id)
        .cloned()
        .with_context(|| format!("Tenant '{}' not found in local tenant store", tenant_id))
}

fn load_session_for_key(session_key: &SessionKey) -> Result<systemprompt_cloud::CliSession> {
    let project_ctx = ProjectContext::discover();

    let (sessions_dir, legacy_path) = if project_ctx.systemprompt_dir().exists() {
        (
            project_ctx.sessions_dir(),
            Some(project_ctx.local_session()),
        )
    } else {
        let cloud_paths = get_cloud_paths().context("Failed to resolve cloud paths")?;
        (
            cloud_paths.resolve(CloudPath::SessionsDir),
            Some(cloud_paths.resolve(CloudPath::CliSession)),
        )
    };

    let store = SessionStore::load_or_create(&sessions_dir, legacy_path.as_deref())?;

    store
        .get_valid_session(session_key)
        .cloned()
        .context("No active session. Run 'systemprompt infra system login'.")
}
