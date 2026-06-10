use anyhow::{Result, bail};
use std::path::PathBuf;
use systemprompt_cloud::ProfilePath;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::TenantId;

pub(in crate::commands::cloud) fn get_tenant_id() -> Result<TenantId> {
    let profile =
        ProfileBootstrap::get().map_err(|_e| anyhow::anyhow!("Profile not initialized"))?;

    let cloud = profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Cloud not configured in profile"))?;

    cloud
        .tenant_id
        .as_ref()
        .map(TenantId::new)
        .ok_or_else(|| anyhow::anyhow!("No tenant_id in profile. Create a cloud tenant first."))
}

pub(in crate::commands::cloud) fn get_tenant_and_secrets_path() -> Result<(TenantId, PathBuf)> {
    let tenant_id = get_tenant_id()?;

    let profile_path =
        ProfileBootstrap::get_path().map_err(|_e| anyhow::anyhow!("Profile path not available"))?;

    let profile_dir = std::path::Path::new(profile_path)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid profile path"))?;

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);

    if !secrets_path.exists() {
        bail!(
            "secrets.json not found at {}. Create it first.",
            secrets_path.display()
        );
    }

    Ok((tenant_id, secrets_path))
}
