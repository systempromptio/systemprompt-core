//! Builder for the cloud-deployment [`Profile`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TenantId;
use systemprompt_models::profile::{
    ProviderRegistry, SecretsConfig, SecretsSource, SecretsValidationMode, TrustedIssuer,
};
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig, ExtensionsConfig, PathsConfig,
    Profile, ProfileDatabaseConfig, ProfileType, RateLimitsConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

use super::{cloud_runtime_config, generate_display_name, security_config, webhook_governance};
use crate::constants::{container, profile as consts};

#[derive(Debug)]
pub struct CloudProfileBuilder {
    name: String,
    tenant_id: Option<TenantId>,
    external_url: Option<String>,
    external_db_access: bool,
    secrets_path: Option<String>,
    trusted_issuers: Vec<TrustedIssuer>,
}

impl CloudProfileBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tenant_id: None,
            external_url: None,
            external_db_access: false,
            secrets_path: None,
            trusted_issuers: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_trusted_issuer(mut self, issuer: TrustedIssuer) -> Self {
        self.trusted_issuers.push(issuer);
        self
    }

    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    #[must_use]
    pub fn with_external_url(mut self, url: impl Into<String>) -> Self {
        self.external_url = Some(url.into());
        self
    }

    #[must_use]
    pub const fn with_external_db_access(mut self, enabled: bool) -> Self {
        self.external_db_access = enabled;
        self
    }

    #[must_use]
    pub fn with_secrets_path(mut self, path: impl Into<String>) -> Self {
        self.secrets_path = Some(path.into());
        self
    }

    #[must_use]
    pub fn build(self) -> Profile {
        let display_name = generate_display_name(&self.name);
        let external = self
            .external_url
            .unwrap_or_else(|| consts::DEFAULT_CLOUD_URL.to_owned());
        let internal_url = format!("http://localhost:{}", consts::DEFAULT_PORT);

        Profile {
            name: self.name,
            display_name,
            target: ProfileType::Cloud,
            site: SiteConfig {
                name: "systemprompt.io".to_owned(),
                github_link: None,
            },
            database: ProfileDatabaseConfig {
                db_type: consts::DEFAULT_DB_TYPE.to_owned(),
                external_db_access: self.external_db_access,
                pool: None,
            },
            server: ServerConfig {
                host: consts::CLOUD_HOST.to_owned(),
                port: consts::DEFAULT_PORT,
                api_server_url: external.clone(),
                api_internal_url: internal_url.clone(),
                api_external_url: external.clone(),
                use_https: true,
                cors_allowed_origins: vec![external],
                content_negotiation: ContentNegotiationConfig::default(),
                security_headers: SecurityHeadersConfig::default(),
                instance_id: None,
                max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
                trusted_proxies: crate::trusted_proxies::default_cloud_trusted_proxies(),
            },
            paths: PathsConfig {
                system: container::APP.to_owned(),
                services: container::SERVICES.to_owned(),
                bin: container::BIN.to_owned(),
                storage: Some(container::STORAGE.to_owned()),
                geoip_database: None,
                web_path: Some(container::WEB.to_owned()),
            },
            security: security_config(consts::CLOUD_ISSUER, self.trusted_issuers),
            rate_limits: RateLimitsConfig::default(),
            runtime: cloud_runtime_config(),
            cloud: Some(CloudConfig {
                tenant_id: self.tenant_id,
                validation: CloudValidationMode::Strict,
            }),
            secrets: Some(SecretsConfig {
                secrets_path: self.secrets_path.unwrap_or_else(String::new),
                validation: SecretsValidationMode::Strict,
                source: SecretsSource::Env,
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
