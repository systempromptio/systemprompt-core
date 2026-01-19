//! CLI routing layer for local vs remote execution.

pub mod remote;

use anyhow::{Context, Result};
use systemprompt_cloud::{
    get_cloud_paths, CloudCredentials, CloudPath, CredentialsBootstrap, ProjectContext,
    StoredTenant, TenantStore,
};
use systemprompt_models::ProfileBootstrap;

pub enum ExecutionTarget {
    Local,
    Remote { hostname: String, token: String },
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

    let credentials = load_credentials()?;

    Ok(ExecutionTarget::Remote {
        hostname,
        token: credentials.api_token,
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

fn load_credentials() -> Result<CloudCredentials> {
    CredentialsBootstrap::get()
        .context("Failed to get credentials")?
        .cloned()
        .context("Cloud credentials required. Run 'systemprompt cloud login'.")
}
