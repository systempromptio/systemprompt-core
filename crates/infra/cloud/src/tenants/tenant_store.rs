//! Persistent map of [`super::StoredTenant`] records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;
use validator::Validate;

use super::StoredTenant;
use crate::api_client::TenantInfo;
use crate::error::{CloudError, CloudResult};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TenantStore {
    #[validate(nested)]
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

    pub fn load_from_path(path: &Path) -> CloudResult<Self> {
        if !path.exists() {
            return Err(CloudError::TenantsNotSynced);
        }

        let content = fs::read_to_string(path)?;

        let store: Self = serde_json::from_str(&content)
            .map_err(|e| CloudError::TenantsStoreCorrupted { source: e })?;

        store
            .validate()
            .map_err(|e| CloudError::TenantsStoreInvalid {
                message: e.to_string(),
            })?;

        Ok(store)
    }

    pub fn save_to_path(&self, path: &Path) -> CloudResult<()> {
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
    pub fn find_tenant(&self, id: &TenantId) -> Option<&StoredTenant> {
        self.tenants.iter().find(|t| t.id == *id)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
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
