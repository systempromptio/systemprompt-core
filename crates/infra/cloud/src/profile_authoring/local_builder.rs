//! Builder for the local-development [`Profile`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::TenantId;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::profile::{
    ProviderRegistry, SecretsConfig, SecretsSource, SecretsValidationMode,
};
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig, ExtensionsConfig, PathsConfig,
    Profile, ProfileDatabaseConfig, ProfileType, RateLimitsConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

use super::{generate_display_name, local_runtime_config, security_config, webhook_governance};
use crate::constants::profile as consts;
use crate::paths::ProjectContext;

#[derive(Debug)]
pub struct LocalProfileBuilder {
    name: String,
    tenant_id: Option<TenantId>,
    secrets_path: String,
    services_path: String,
}

impl LocalProfileBuilder {
    pub fn new(
        name: impl Into<String>,
        secrets_path: impl AsRef<Path>,
        services_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            name: name.into(),
            tenant_id: None,
            secrets_path: secrets_path.as_ref().to_string_lossy().to_string(),
            services_path: services_path.as_ref().to_string_lossy().to_string(),
        }
    }

    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    #[must_use]
    pub fn build(self) -> Profile {
        let ctx = ProjectContext::discover();
        let root = ctx.root();
        let system_path = root.to_string_lossy().to_string();
        let display_name = generate_display_name(&self.name);
        let local_url = format!("http://localhost:{}", consts::DEFAULT_PORT);
        let internal_url = local_url.clone();

        Profile {
            name: self.name,
            display_name,
            target: ProfileType::Local,
            site: SiteConfig {
                name: "systemprompt.io".to_owned(),
                github_link: None,
            },
            database: ProfileDatabaseConfig {
                db_type: consts::DEFAULT_DB_TYPE.to_owned(),
                external_db_access: false,
                pool: None,
            },
            server: ServerConfig {
                host: consts::LOCAL_HOST.to_owned(),
                port: consts::DEFAULT_PORT,
                api_server_url: local_url.clone(),
                api_internal_url: local_url.clone(),
                api_external_url: local_url.clone(),
                use_https: false,
                cors_allowed_origins: vec![local_url, "http://localhost:5173".to_owned()],
                content_negotiation: ContentNegotiationConfig::default(),
                security_headers: SecurityHeadersConfig::default(),
                instance_id: None,
                max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
                trusted_proxies: Vec::new(),
            },
            paths: PathsConfig {
                system: system_path,
                services: self.services_path,
                bin: ExtensionLoader::resolve_bin_directory(root, None)
                    .to_string_lossy()
                    .to_string(),
                storage: Some(ctx.storage_dir().to_string_lossy().to_string()),
                geoip_database: None,
                web_path: None,
            },
            security: security_config(consts::LOCAL_ISSUER, Vec::new()),
            rate_limits: RateLimitsConfig {
                disabled: true,
                ..Default::default()
            },
            runtime: local_runtime_config(),
            cloud: Some(CloudConfig {
                tenant_id: self.tenant_id,
                validation: CloudValidationMode::Warn,
            }),
            secrets: Some(SecretsConfig {
                secrets_path: self.secrets_path,
                validation: SecretsValidationMode::Warn,
                source: SecretsSource::File,
            }),
            extensions: ExtensionsConfig::default(),
            providers: ProviderRegistry::default(),
            gateway: None,
            governance: Some(webhook_governance(&internal_url)),
            system_admin: SystemAdminConfig {
                username: "admin".to_owned(),
            },
        }
    }
}
