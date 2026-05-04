//! Persistent map of [`super::StoredTenant`] records.

use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

use super::StoredTenant;
use crate::api_client::TenantInfo;
use crate::error::{CloudError, CloudResult};

/// Versioned, validated tenants index persisted alongside cloud
/// credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TenantStore {
    /// Tenants known to this CLI installation.
    #[validate(nested)]
    pub tenants: Vec<StoredTenant>,

    /// Wall-clock time the store was last refreshed from the cloud.
    pub synced_at: DateTime<Utc>,
}

impl TenantStore {
    /// Create a fresh store with `synced_at` set to now.
    #[must_use]
    pub fn new(tenants: Vec<StoredTenant>) -> Self {
        Self {
            tenants,
            synced_at: Utc::now(),
        }
    }

    /// Build a store from cloud-side [`TenantInfo`] payloads.
    #[must_use]
    pub fn from_tenant_infos(infos: &[TenantInfo]) -> Self {
        let tenants = infos.iter().map(StoredTenant::from_tenant_info).collect();
        Self::new(tenants)
    }

    /// Read and validate the on-disk tenants store.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TenantsNotSynced`] when the file does
    /// not exist, [`CloudError::TenantsStoreCorrupted`] for parse
    /// failures, and [`CloudError::TenantsStoreInvalid`] for
    /// validator failures.
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

    /// Validate and write the store to disk with `0o600`
    /// permissions on Unix.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TenantsStoreInvalid`] when validation
    /// fails before writing, plus the usual I/O / JSON errors.
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

    /// Find a tenant by id.
    #[must_use]
    pub fn find_tenant(&self, id: &str) -> Option<&StoredTenant> {
        self.tenants.iter().find(|t| t.id == id)
    }

    /// `true` if the store has no tenants.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tenants.is_empty()
    }

    /// Number of tenants in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tenants.len()
    }

    /// `true` if the store was last synced more than `max_age` ago.
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
