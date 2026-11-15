//! Tenant-type selection (local or cloud) during profile creation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};
use systemprompt_cloud::{StoredTenant, TenantStore};

use super::TenantTypeArg;

use crate::interactive::Prompter;

pub fn select_tenant_type(prompter: &dyn Prompter, store: &TenantStore) -> Result<TenantTypeArg> {
    let local_count = store
        .tenants
        .iter()
        .filter(|t| TenantTypeArg::Local.admits(t))
        .count();
    let cloud_count = store
        .tenants
        .iter()
        .filter(|t| TenantTypeArg::Cloud.admits(t))
        .count();

    let local_label = match local_count {
        0 => "Local - no tenants available".to_owned(),
        1 => "Local - 1 tenant available".to_owned(),
        n => format!("Local - {} tenants available", n),
    };

    let cloud_label = match cloud_count {
        0 => "Cloud - no tenants available".to_owned(),
        1 => "Cloud - 1 tenant available".to_owned(),
        n => format!("Cloud - {} tenants available", n),
    };

    let options = vec![local_label, cloud_label];

    let selection = prompter.select("Profile type", &options)?;

    if selection == 0 {
        if local_count == 0 {
            bail!(
                "No local tenants available.\nRun 'systemprompt cloud tenant create' and select \
                 'Local' to create one."
            );
        }
        Ok(TenantTypeArg::Local)
    } else {
        if cloud_count == 0 {
            bail!(
                "No cloud tenants available.\nRun 'systemprompt cloud tenant create' and select \
                 'Cloud' to create one."
            );
        }
        Ok(TenantTypeArg::Cloud)
    }
}

pub(super) fn get_tenants_by_type(
    store: &TenantStore,
    tenant_type: TenantTypeArg,
) -> Vec<StoredTenant> {
    store
        .tenants
        .iter()
        .filter(|t| tenant_type.admits(t))
        .cloned()
        .collect()
}

pub fn select_tenant(prompter: &dyn Prompter, tenants: &[StoredTenant]) -> Result<StoredTenant> {
    if tenants.is_empty() {
        bail!("No eligible tenants found.");
    }

    let options: Vec<String> = tenants
        .iter()
        .map(|t| {
            let db_status = if t.has_database_url() {
                "✓ db"
            } else {
                "✗ db"
            };
            format!("{} [{}]", t.name, db_status)
        })
        .collect();

    let selection = prompter.select("Select tenant", &options)?;

    Ok(tenants[selection].clone())
}
