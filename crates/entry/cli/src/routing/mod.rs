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
        tracing::debug!("No profile loaded, routing to local execution");
        return Ok(ExecutionTarget::Local);
    };

    if profile.target.is_local() {
        tracing::debug!(
            profile_name = %profile.name,
            "Profile target is local, routing to local execution"
        );
        return Ok(ExecutionTarget::Local);
    }

    let Some(tenant_id) = profile.cloud.as_ref().and_then(|c| c.tenant_id.as_ref()) else {
        tracing::debug!(
            profile_name = %profile.name,
            "Profile has no tenant_id, routing to local execution"
        );
        return Ok(ExecutionTarget::Local);
    };

    tracing::debug!(
        profile_name = %profile.name,
        tenant_id = %tenant_id,
        "Profile has tenant_id, resolving remote execution target"
    );

    let tenant = resolve_tenant(tenant_id)?;
    let hostname = tenant
        .hostname
        .as_ref()
        .context("Tenant has no hostname configured")?
        .clone();

    let session_key = SessionKey::Tenant(TenantId::new(tenant_id.clone()));
    let session = load_session_for_key(&session_key)?;

    tracing::info!(
        hostname = %hostname,
        tenant_id = %tenant_id,
        "Routing to remote execution"
    );

    Ok(ExecutionTarget::Remote {
        hostname,
        token: session.session_token.as_str().to_string(),
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

    let sessions_dir = if project_ctx.systemprompt_dir().exists() {
        project_ctx.sessions_dir()
    } else {
        let cloud_paths = get_cloud_paths().context("Failed to resolve cloud paths")?;
        cloud_paths.resolve(CloudPath::SessionsDir)
    };

    let store = SessionStore::load_or_create(&sessions_dir)?;

    store
        .get_valid_session(session_key)
        .cloned()
        .context("No active session. Run 'systemprompt admin session login'.")
}
