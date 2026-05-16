//! Sync configuration: the [`SyncConfig`] value passed to
//! [`crate::SyncService`] and its [`SyncConfigBuilder`].

use systemprompt_identifiers::TenantId;

use crate::SyncDirection;

#[derive(Clone, Debug)]
pub struct SyncConfig {
    pub direction: SyncDirection,
    pub dry_run: bool,
    pub verbose: bool,
    pub tenant_id: TenantId,
    pub api_url: String,
    pub api_token: String,
    pub services_path: String,
    pub hostname: Option<String>,
    pub sync_token: Option<String>,
    pub local_database_url: Option<String>,
}

#[derive(Debug)]
pub struct SyncConfigBuilder {
    direction: SyncDirection,
    dry_run: bool,
    verbose: bool,
    tenant_id: TenantId,
    api_url: String,
    api_token: String,
    services_path: String,
    hostname: Option<String>,
    sync_token: Option<String>,
    local_database_url: Option<String>,
}

impl SyncConfigBuilder {
    pub fn new(
        tenant_id: impl Into<TenantId>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> Self {
        Self {
            direction: SyncDirection::Push,
            dry_run: false,
            verbose: false,
            tenant_id: tenant_id.into(),
            api_url: api_url.into(),
            api_token: api_token.into(),
            services_path: services_path.into(),
            hostname: None,
            sync_token: None,
            local_database_url: None,
        }
    }

    pub const fn with_direction(mut self, direction: SyncDirection) -> Self {
        self.direction = direction;
        self
    }

    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_hostname(mut self, hostname: Option<String>) -> Self {
        self.hostname = hostname;
        self
    }

    pub fn with_sync_token(mut self, sync_token: Option<String>) -> Self {
        self.sync_token = sync_token;
        self
    }

    pub fn with_local_database_url(mut self, url: impl Into<String>) -> Self {
        self.local_database_url = Some(url.into());
        self
    }

    pub fn build(self) -> SyncConfig {
        SyncConfig {
            direction: self.direction,
            dry_run: self.dry_run,
            verbose: self.verbose,
            tenant_id: self.tenant_id,
            api_url: self.api_url,
            api_token: self.api_token,
            services_path: self.services_path,
            hostname: self.hostname,
            sync_token: self.sync_token,
            local_database_url: self.local_database_url,
        }
    }
}

impl SyncConfig {
    pub fn builder(
        tenant_id: impl Into<TenantId>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> SyncConfigBuilder {
        SyncConfigBuilder::new(tenant_id, api_url, api_token, services_path)
    }
}
