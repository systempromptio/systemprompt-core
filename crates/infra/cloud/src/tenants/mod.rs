//! On-disk representation of cloud tenants the CLI knows about.
//!
//! [`StoredTenant`] is the per-tenant record; [`TenantStore`] (in
//! `tenant_store.rs`) is the persistent map keyed by tenant id.

mod tenant_store;

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::api_client::TenantInfo;

pub use tenant_store::TenantStore;

/// Constructor parameters for [`StoredTenant::new_cloud`].
#[derive(Debug)]
pub struct NewCloudTenantParams {
    /// Cloud tenant id.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Fly.io app id, when known.
    pub app_id: Option<String>,
    /// Provisioned hostname.
    pub hostname: Option<String>,
    /// Cloud region.
    pub region: Option<String>,
    /// Public-facing connection string.
    pub database_url: Option<String>,
    /// Internal connection string used by the tenant container.
    pub internal_database_url: String,
    /// `true` when the tenant has external DB access enabled.
    pub external_db_access: bool,
    /// Sync token, when issued.
    pub sync_token: Option<String>,
}

/// Whether a tenant lives in cloud or runs locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TenantType {
    /// Tenant is rooted in a local profile.
    #[default]
    Local,
    /// Tenant is provisioned in systemprompt.io Cloud.
    Cloud,
}

/// One row of the on-disk tenants index.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct StoredTenant {
    /// Tenant identifier.
    #[validate(length(min = 1, message = "Tenant ID cannot be empty"))]
    pub id: String,

    /// Display name.
    #[validate(length(min = 1, message = "Tenant name cannot be empty"))]
    pub name: String,

    /// Optional Fly.io app id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,

    /// Optional provisioned hostname.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    /// Optional cloud region.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    /// Optional public-facing connection string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    /// Optional internal connection string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_database_url: Option<String>,

    /// Whether this tenant is local or cloud.
    #[serde(default)]
    pub tenant_type: TenantType,

    /// Whether external DB access is enabled.
    #[serde(default)]
    pub external_db_access: bool,

    /// Sync token for the cloud → local replication pipeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,

    /// Shared container database URL (for local-shared tenants).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_container_db: Option<String>,
}

impl StoredTenant {
    /// Construct a bare tenant record with only id and name set.
    #[must_use]
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: None,
            internal_database_url: None,
            tenant_type: TenantType::default(),
            external_db_access: false,
            sync_token: None,
            shared_container_db: None,
        }
    }

    /// Construct a local tenant with a dedicated database URL.
    #[must_use]
    pub const fn new_local(id: String, name: String, database_url: String) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: Some(database_url),
            internal_database_url: None,
            tenant_type: TenantType::Local,
            external_db_access: false,
            sync_token: None,
            shared_container_db: None,
        }
    }

    /// Construct a local tenant that lives inside the shared
    /// container database.
    #[must_use]
    pub const fn new_local_shared(
        id: String,
        name: String,
        database_url: String,
        shared_container_db: String,
    ) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: Some(database_url),
            internal_database_url: None,
            tenant_type: TenantType::Local,
            external_db_access: false,
            sync_token: None,
            shared_container_db: Some(shared_container_db),
        }
    }

    /// Construct a cloud tenant from [`NewCloudTenantParams`].
    #[must_use]
    pub fn new_cloud(params: NewCloudTenantParams) -> Self {
        Self {
            id: params.id,
            name: params.name,
            app_id: params.app_id,
            hostname: params.hostname,
            region: params.region,
            database_url: params.database_url,
            internal_database_url: Some(params.internal_database_url),
            tenant_type: TenantType::Cloud,
            external_db_access: params.external_db_access,
            sync_token: params.sync_token,
            shared_container_db: None,
        }
    }

    /// Convert a cloud-side [`TenantInfo`] response into a stored
    /// tenant record.
    #[must_use]
    pub fn from_tenant_info(info: &TenantInfo) -> Self {
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            app_id: info.app_id.clone(),
            hostname: info.hostname.clone(),
            region: info.region.clone(),
            database_url: None,
            internal_database_url: Some(info.database_url.clone()),
            tenant_type: TenantType::Cloud,
            external_db_access: info.external_db_access,
            sync_token: None,
            shared_container_db: None,
        }
    }

    /// `true` if the tenant uses the shared container database.
    #[must_use]
    pub const fn uses_shared_container(&self) -> bool {
        self.shared_container_db.is_some()
    }

    /// `true` if the tenant has any non-empty database URL.
    #[must_use]
    pub fn has_database_url(&self) -> bool {
        match self.tenant_type {
            TenantType::Cloud => self
                .internal_database_url
                .as_ref()
                .is_some_and(|url| !url.is_empty()),
            TenantType::Local => self
                .database_url
                .as_ref()
                .is_some_and(|url| !url.is_empty()),
        }
    }

    /// Borrow the local-side database URL, falling back to the
    /// internal URL when unset.
    #[must_use]
    pub fn get_local_database_url(&self) -> Option<&String> {
        self.database_url
            .as_ref()
            .or(self.internal_database_url.as_ref())
    }

    /// `true` if `tenant_type == Cloud`.
    #[must_use]
    pub const fn is_cloud(&self) -> bool {
        matches!(self.tenant_type, TenantType::Cloud)
    }

    /// `true` if `tenant_type == Local`.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self.tenant_type, TenantType::Local)
    }

    /// Update mutable fields from a fresh [`TenantInfo`] payload.
    pub fn update_from_tenant_info(&mut self, info: &TenantInfo) {
        self.name.clone_from(&info.name);
        self.app_id.clone_from(&info.app_id);
        self.hostname.clone_from(&info.hostname);
        self.region.clone_from(&info.region);
        self.external_db_access = info.external_db_access;

        if !info.database_url.contains(":***@") {
            self.internal_database_url = Some(info.database_url.clone());
        }
    }

    /// `true` if this is a cloud tenant without a sync token.
    #[must_use]
    pub fn is_sync_token_missing(&self) -> bool {
        self.tenant_type == TenantType::Cloud && self.sync_token.is_none()
    }

    /// `true` if the internal database URL has been redacted by the
    /// cloud API.
    #[must_use]
    pub fn is_database_url_masked(&self) -> bool {
        self.internal_database_url
            .as_ref()
            .is_some_and(|url| url.contains(":***@") || url.contains(":********@"))
    }

    /// `true` if any required cloud credential is missing or
    /// redacted.
    #[must_use]
    pub fn has_missing_credentials(&self) -> bool {
        self.tenant_type == TenantType::Cloud
            && (self.is_sync_token_missing() || self.is_database_url_masked())
    }
}
