//! On-disk representation of the tenants the CLI knows about.
//!
//! [`StoredTenant`] is the per-tenant record; [`TenantStore`] (in
//! `tenant_store.rs`) is the persistent map keyed by tenant id. Each record
//! names its [`HostingProvider`]: only `systemprompt` tenants are reconciled
//! with the control plane, every other record is self-managed and written by
//! `cloud tenant add`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod tenant_store;

use serde::{Deserialize, Deserializer, Serialize};
use systemprompt_identifiers::{CloudAppId, TenantId};
use systemprompt_manifest::HostingProvider;
use validator::Validate;

use crate::api_client::CloudTenantInfo;

pub use tenant_store::TenantStore;

#[derive(Debug)]
pub struct NewCloudTenantParams {
    pub id: TenantId,
    pub name: String,
    pub app_id: Option<CloudAppId>,
    pub hostname: Option<String>,
    pub region: Option<String>,
    pub database_url: Option<String>,
    pub internal_database_url: String,
    pub external_db_access: bool,
}

#[derive(Debug)]
pub struct NewSelfManagedTenantParams {
    pub id: TenantId,
    pub name: String,
    pub provider: HostingProvider,
    pub hostname: String,
    pub region: Option<String>,
    pub database_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct StoredTenant {
    pub id: TenantId,

    #[validate(length(min = 1, message = "Tenant name cannot be empty"))]
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<CloudAppId>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_database_url: Option<String>,

    #[serde(
        alias = "tenant_type",
        default = "absent_provider",
        deserialize_with = "deserialize_provider"
    )]
    pub provider: HostingProvider,

    #[serde(default)]
    pub external_db_access: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker_project: Option<String>,
}

impl StoredTenant {
    #[must_use]
    pub const fn new(id: TenantId, name: String) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: None,
            internal_database_url: None,
            provider: HostingProvider::DockerLocal,
            external_db_access: false,
            docker_project: None,
        }
    }

    #[must_use]
    pub const fn new_local(id: TenantId, name: String, database_url: String) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: Some(database_url),
            internal_database_url: None,
            provider: HostingProvider::DockerLocal,
            external_db_access: false,
            docker_project: None,
        }
    }

    #[must_use]
    pub const fn new_local_docker(
        id: TenantId,
        name: String,
        database_url: String,
        docker_project: String,
    ) -> Self {
        Self {
            id,
            name,
            app_id: None,
            hostname: None,
            region: None,
            database_url: Some(database_url),
            internal_database_url: None,
            provider: HostingProvider::DockerLocal,
            external_db_access: false,
            docker_project: Some(docker_project),
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
            provider: HostingProvider::Systemprompt,
            external_db_access: params.external_db_access,
            docker_project: None,
        }
    }

    #[must_use]
    pub fn from_tenant_info(info: &CloudTenantInfo) -> Self {
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            app_id: info.app_id.clone(),
            hostname: info.hostname.clone(),
            region: info.region.clone(),
            database_url: None,
            internal_database_url: Some(info.database_url.clone()),
            provider: HostingProvider::Systemprompt,
            external_db_access: info.external_db_access,
            docker_project: None,
        }
    }

    #[must_use]
    pub fn new_self_managed(params: NewSelfManagedTenantParams) -> Self {
        Self {
            id: params.id,
            name: params.name,
            app_id: None,
            hostname: Some(params.hostname),
            region: params.region,
            database_url: params.database_url,
            internal_database_url: None,
            provider: params.provider,
            external_db_access: false,
            docker_project: None,
        }
    }

    #[must_use]
    pub const fn uses_managed_container(&self) -> bool {
        self.docker_project.is_some()
    }

    #[must_use]
    pub fn has_database_url(&self) -> bool {
        let url = if self.provider.is_systemprompt() {
            self.internal_database_url.as_ref()
        } else {
            self.database_url.as_ref()
        };
        url.is_some_and(|url| !url.is_empty())
    }

    #[must_use]
    pub fn get_local_database_url(&self) -> Option<&String> {
        self.database_url
            .as_ref()
            .or(self.internal_database_url.as_ref())
    }

    #[must_use]
    pub const fn is_systemprompt_managed(&self) -> bool {
        self.provider.is_systemprompt()
    }

    #[must_use]
    pub const fn is_self_managed(&self) -> bool {
        !self.provider.is_systemprompt()
    }

    #[must_use]
    pub const fn is_local(&self) -> bool {
        self.provider.is_docker_local()
    }

    pub fn update_from_tenant_info(&mut self, info: &CloudTenantInfo) {
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
        self.provider.is_systemprompt() && self.is_database_url_masked()
    }
}

const fn absent_provider() -> HostingProvider {
    HostingProvider::DockerLocal
}

fn deserialize_provider<'de, D>(deserializer: D) -> Result<HostingProvider, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    match raw.as_str() {
        "cloud" => Ok(HostingProvider::Systemprompt),
        "local" => Ok(HostingProvider::DockerLocal),
        other => other.parse().map_err(serde::de::Error::custom),
    }
}
