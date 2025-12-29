use anyhow::{bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{StoredTenant, TenantStore, TenantType};

pub fn select_tenant_type(store: &TenantStore) -> Result<TenantType> {
    let local_count = store
        .tenants
        .iter()
        .filter(|t| t.tenant_type == TenantType::Local)
        .count();
    let cloud_count = store
        .tenants
        .iter()
        .filter(|t| t.tenant_type == TenantType::Cloud)
        .count();

    let local_label = match local_count {
        0 => "Local - no tenants available".to_string(),
        1 => "Local - 1 tenant available".to_string(),
        n => format!("Local - {} tenants available", n),
    };

    let cloud_label = match cloud_count {
        0 => "Cloud - no tenants available".to_string(),
        1 => "Cloud - 1 tenant available".to_string(),
        n => format!("Cloud - {} tenants available", n),
    };

    let options = vec![local_label, cloud_label];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile type")
        .items(&options)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            if local_count == 0 {
                bail!(
                    "No local tenants available.\nRun 'systemprompt cloud tenant create' and \
                     select 'Local' to create one."
                );
            }
            Ok(TenantType::Local)
        },
        _ => {
            if cloud_count == 0 {
                bail!(
                    "No cloud tenants available.\nRun 'systemprompt cloud tenant create' and \
                     select 'Cloud' to create one."
                );
            }
            Ok(TenantType::Cloud)
        },
    }
}

pub fn get_tenants_by_type(store: &TenantStore, tenant_type: TenantType) -> Vec<StoredTenant> {
    store
        .tenants
        .iter()
        .filter(|t| t.tenant_type == tenant_type)
        .cloned()
        .collect()
}

pub fn select_tenant(tenants: &[StoredTenant]) -> Result<StoredTenant> {
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

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select tenant")
        .items(&options)
        .default(0)
        .interact()?;

    Ok(tenants[selection].clone())
}
