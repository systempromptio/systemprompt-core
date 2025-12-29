use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use validator::Validate;

use crate::api_client::TenantInfo;
use crate::error::CloudError;

#[derive(Debug)]
pub struct NewCloudTenantParams {
    pub id: String,
    pub name: String,
    pub app_id: Option<String>,
    pub hostname: Option<String>,
    pub region: Option<String>,
    pub database_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TenantType {
    #[default]
    Local,
    Cloud,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct StoredTenant {
    #[validate(length(min = 1, message = "Tenant ID cannot be empty"))]
    pub id: String,

    #[validate(length(min = 1, message = "Tenant name cannot be empty"))]
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    #[serde(default)]
    pub tenant_type: TenantType,
}

impl StoredTenant {
    #[must_use]
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: None,
            tenant_type: TenantType::default(),
        }
    }

    #[must_use]
    pub const fn new_local(id: String, name: String, database_url: String) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: Some(database_url),
            tenant_type: TenantType::Local,
        }
    }

    #[must_use]
    pub fn new_cloud(params: NewCloudTenantParams) -> Self {
        Self {
            id: params.id,
            name: params.name,
            app_id: params.app_id,
            hostname: params.hostname,
            region: params.region,
            database_url: Some(params.database_url),
            tenant_type: TenantType::Cloud,
        }
    }

    #[must_use]
    pub fn from_tenant_info(info: &TenantInfo) -> Self {
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            app_id: info.app_id.clone(),
            hostname: info.hostname.clone(),
            region: info.region.clone(),
            database_url: None,
            tenant_type: TenantType::Cloud,
        }
    }

    #[must_use]
    pub fn has_database_url(&self) -> bool {
        self.database_url
            .as_ref()
            .is_some_and(|url| !url.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TenantStore {
    #[validate]
    pub tenants: Vec<StoredTenant>,

    pub synced_at: DateTime<Utc>,
}

impl TenantStore {
    #[must_use]
    pub fn new(tenants: Vec<StoredTenant>) -> Self {
        Self {
            tenants,
            synced_at: Utc::now(),
        }
    }

    #[must_use]
    pub fn from_tenant_infos(infos: &[TenantInfo]) -> Self {
        let tenants = infos.iter().map(StoredTenant::from_tenant_info).collect();
        Self::new(tenants)
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(CloudError::TenantsNotSynced.into());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let store: Self = serde_json::from_str(&content)
            .map_err(|e| CloudError::TenantsStoreCorrupted { source: e })?;

        store
            .validate()
            .map_err(|e| CloudError::TenantsStoreInvalid {
                message: e.to_string(),
            })?;

        Ok(store)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        self.validate()
            .map_err(|e| CloudError::TenantsStoreInvalid {
                message: e.to_string(),
            })?;

        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;

            let gitignore_path = dir.join(".gitignore");
            if !gitignore_path.exists() {
                fs::write(&gitignore_path, "*\n")?;
            }
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    #[must_use]
    pub fn find_tenant(&self, id: &str) -> Option<&StoredTenant> {
        self.tenants.iter().find(|t| t.id == id)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tenants.len()
    }

    #[must_use]
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        let age = Utc::now() - self.synced_at;
        age > max_age
    }
}

impl Default for TenantStore {
    fn default() -> Self {
        Self {
            tenants: Vec::new(),
            synced_at: Utc::now(),
        }
    }
}
