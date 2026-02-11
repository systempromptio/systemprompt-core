use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::Profile;

use crate::api_client::CloudApiClient;
use crate::credentials::CloudCredentials;
use crate::error::{CloudError, CloudResult};
use crate::paths::{get_cloud_paths, CloudPath};
use crate::tenants::{StoredTenant, TenantStore};

#[derive(Debug, Clone)]
pub struct ResolvedTenant {
    pub id: String,
    pub name: String,
    pub app_id: Option<String>,
    pub hostname: Option<String>,
    pub region: Option<String>,
}

impl From<StoredTenant> for ResolvedTenant {
    fn from(tenant: StoredTenant) -> Self {
        Self {
            id: tenant.id,
            name: tenant.name,
            app_id: tenant.app_id,
            hostname: tenant.hostname,
            region: tenant.region,
        }
    }
}

#[derive(Debug)]
pub struct CloudContext {
    pub credentials: CloudCredentials,
    pub profile: Option<&'static Profile>,
    pub tenant: Option<ResolvedTenant>,
    pub api_client: CloudApiClient,
}

impl CloudContext {
    pub fn new_authenticated() -> CloudResult<Self> {
        let cloud_paths = get_cloud_paths().map_err(|_| CloudError::NotAuthenticated)?;
        let creds_path = cloud_paths.resolve(CloudPath::Credentials);
        let credentials = CloudCredentials::load_and_validate_from_path(&creds_path)
            .map_err(|_| CloudError::NotAuthenticated)?;

        let api_client = CloudApiClient::new(&credentials.api_url, &credentials.api_token)
            .map_err(CloudError::Network)?;

        Ok(Self {
            credentials,
            profile: None,
            tenant: None,
            api_client,
        })
    }

    pub fn with_profile(mut self) -> CloudResult<Self> {
        let profile = ProfileBootstrap::get().map_err(|e| CloudError::ProfileRequired {
            message: e.to_string(),
        })?;

        self.profile = Some(profile);

        if let Some(ref cloud_config) = profile.cloud {
            if let Some(ref tenant_id) = cloud_config.tenant_id {
                self.tenant = Self::resolve_tenant(tenant_id)?;
            }
        }

        Ok(self)
    }

    fn resolve_tenant(tenant_id: &str) -> CloudResult<Option<ResolvedTenant>> {
        let cloud_paths = get_cloud_paths().map_err(|_| CloudError::TenantsNotSynced)?;
        let tenants_path = cloud_paths.resolve(CloudPath::Tenants);

        if !tenants_path.exists() {
            return Ok(None);
        }

        let store =
            TenantStore::load_from_path(&tenants_path).map_err(|_| CloudError::TenantsNotSynced)?;

        store.find_tenant(tenant_id).map_or_else(
            || {
                Err(CloudError::TenantNotFound {
                    tenant_id: tenant_id.to_string(),
                })
            },
            |tenant| Ok(Some(ResolvedTenant::from(tenant.clone()))),
        )
    }

    pub fn tenant_id(&self) -> CloudResult<&str> {
        self.tenant
            .as_ref()
            .map(|t| t.id.as_str())
            .ok_or(CloudError::TenantNotConfigured)
    }

    pub fn app_id(&self) -> CloudResult<&str> {
        self.tenant
            .as_ref()
            .and_then(|t| t.app_id.as_deref())
            .ok_or(CloudError::AppNotConfigured)
    }

    #[must_use]
    pub fn tenant_name(&self) -> &str {
        self.tenant.as_ref().map_or("unknown", |t| t.name.as_str())
    }

    #[must_use]
    pub fn hostname(&self) -> Option<&str> {
        self.tenant.as_ref().and_then(|t| t.hostname.as_deref())
    }

    pub fn profile(&self) -> CloudResult<&'static Profile> {
        self.profile.ok_or_else(|| CloudError::ProfileRequired {
            message: "Profile not loaded in context".into(),
        })
    }

    #[must_use]
    pub const fn has_tenant(&self) -> bool {
        self.tenant.is_some()
    }
}
