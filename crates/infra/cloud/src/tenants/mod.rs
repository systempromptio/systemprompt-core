//! On-disk representation of cloud tenants the CLI knows about.
//!
//! [`StoredTenant`] is the per-tenant record; [`TenantStore`] (in
//! `tenant_store.rs`) is the persistent map keyed by tenant id.

mod tenant_store;

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::api_client::TenantInfo;

pub use tenant_store::TenantStore;

#[derive(Debug)]
pub struct NewCloudTenantParams {
    pub id: String,
    pub name: String,
    pub app_id: Option<String>,
    pub hostname: Option<String>,
    pub region: Option<String>,
    pub database_url: Option<String>,
    pub internal_database_url: String,
    pub external_db_access: bool,
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_database_url: Option<String>,

    #[serde(default)]
    pub tenant_type: TenantType,

    #[serde(default)]
    pub external_db_access: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_container_db: Option<String>,
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
            internal_database_url: None,
            tenant_type: TenantType::default(),
            external_db_access: false,
            shared_container_db: None,
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
            internal_database_url: None,
            tenant_type: TenantType::Local,
            external_db_access: false,
            shared_container_db: None,
        }
    }

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
            shared_container_db: Some(shared_container_db),
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
            database_url: params.database_url,
            internal_database_url: Some(params.internal_database_url),
            tenant_type: TenantType::Cloud,
            external_db_access: params.external_db_access,
            shared_container_db: None,
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
            internal_database_url: Some(info.database_url.clone()),
            tenant_type: TenantType::Cloud,
            external_db_access: info.external_db_access,
            shared_container_db: None,
        }
    }

    #[must_use]
    pub const fn uses_shared_container(&self) -> bool {
        self.shared_container_db.is_some()
    }

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

    #[must_use]
    pub fn get_local_database_url(&self) -> Option<&String> {
        self.database_url
            .as_ref()
            .or(self.internal_database_url.as_ref())
    }

    #[must_use]
    pub const fn is_cloud(&self) -> bool {
        matches!(self.tenant_type, TenantType::Cloud)
    }

    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self.tenant_type, TenantType::Local)
    }

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

    #[must_use]
    pub fn is_database_url_masked(&self) -> bool {
        self.internal_database_url
            .as_ref()
            .is_some_and(|url| url.contains(":***@") || url.contains(":********@"))
    }

    #[must_use]
    pub fn has_missing_credentials(&self) -> bool {
        self.tenant_type == TenantType::Cloud && self.is_database_url_masked()
    }
}
