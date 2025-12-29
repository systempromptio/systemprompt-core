use anyhow::{anyhow, bail, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{
    get_cloud_paths, CloudCredentials, CloudPath, CredentialsBootstrap, StoredTenant, TenantType,
};

pub fn get_credentials() -> Result<CloudCredentials> {
    if CredentialsBootstrap::is_initialized() {
        return CredentialsBootstrap::require()
            .map_err(|e| {
                anyhow!(
                    "Credentials required. Run 'systemprompt cloud login': {}",
                    e
                )
            })?
            .clone()
            .pipe(Ok);
    }

    let cloud_paths = get_cloud_paths()?;
    let creds_path = cloud_paths.resolve(CloudPath::Credentials);

    if creds_path.exists() {
        CloudCredentials::load_from_path(&creds_path)
    } else {
        bail!("Not logged in. Run 'systemprompt cloud login' first.")
    }
}

pub fn select_tenant(tenants: &[StoredTenant]) -> Result<&StoredTenant> {
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

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select tenant")
        .items(&options)
        .default(0)
        .interact()?;

    Ok(&tenants[selection])
}

trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}
