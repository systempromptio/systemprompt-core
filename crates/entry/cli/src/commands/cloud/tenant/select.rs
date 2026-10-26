//! Interactive tenant selection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_cloud::{CloudCredentials, CredentialsBootstrap, StoredTenant, TenantType};
use systemprompt_identifiers::TenantId;
use systemprompt_manifest::Profile;

use crate::interactive::Prompter;

pub fn get_credentials() -> Result<CloudCredentials> {
    CredentialsBootstrap::read_unvalidated()
        .map_err(|e| anyhow!("Credentials required. Run 'systemprompt cloud auth login': {e}"))?
        .ok_or_else(|| anyhow!("Not logged in. Run 'systemprompt cloud auth login' first."))
}

pub fn select_tenant<'a>(
    prompter: &dyn Prompter,
    tenants: &'a [StoredTenant],
) -> Result<&'a StoredTenant> {
    let options: Vec<String> = tenants
        .iter()
        .map(|t| {
            let type_str = match t.tenant_type {
                TenantType::Local => "local",
                TenantType::Cloud => "cloud",
            };
            format!("{} ({})", t.name, type_str)
        })
        .collect();

    let selection = prompter.select("Select tenant", &options)?;

    Ok(&tenants[selection])
}

pub fn resolve_tenant_id(tenant: Option<TenantId>, profile: Option<&Profile>) -> Result<TenantId> {
    if let Some(id) = tenant {
        return Ok(id);
    }

    profile
        .and_then(|p| p.cloud.as_ref()?.tenant_id.clone())
        .ok_or_else(|| {
            anyhow!("No tenant specified. Use --tenant or configure a tenant in your profile.")
        })
}
