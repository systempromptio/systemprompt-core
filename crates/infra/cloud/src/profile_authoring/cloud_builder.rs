//! Builder for the cloud-deployment [`Profile`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use ipnet::IpNet;
use systemprompt_identifiers::{Email, TenantId};
use systemprompt_manifest::profile::{
    ProfileError, ProfileResult, SecretsConfig, SecretsSource, SecretsValidationMode, TrustedIssuer,
};
use systemprompt_manifest::services::SystemAdminConfig;
use systemprompt_manifest::{
    CloudConfig, CloudValidationMode, ExtensionsConfig, HostingProvider, PathsConfig, Profile,
    ProfileDatabaseConfig, ProfileType, RateLimitsConfig, ServerConfig, SiteConfig,
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
    geoip_database: Option<String>,
    provider: HostingProvider,
    trusted_proxies: Option<Vec<IpNet>>,
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
            geoip_database: None,
            provider: HostingProvider::Systemprompt,
            trusted_proxies: None,
        }
    }

    #[must_use]
    pub const fn with_provider(mut self, provider: HostingProvider) -> Self {
        self.provider = provider;
        self
    }

    #[must_use]
    pub fn with_trusted_proxies(mut self, trusted_proxies: Vec<IpNet>) -> Self {
        self.trusted_proxies = Some(trusted_proxies);
        self
    }

    #[must_use]
    pub fn with_geoip_database(mut self, geoip_database: Option<String>) -> Self {
        self.geoip_database = geoip_database;
        self
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

    pub fn build(self) -> ProfileResult<Profile> {
        let trusted_proxies = self.resolve_trusted_proxies()?;
        let display_name = generate_display_name(&self.name);
        let external = self
            .external_url
            .unwrap_or_else(|| consts::DEFAULT_CLOUD_URL.to_owned());

        let mut profile = Profile {
            storage: systemprompt_manifest::profile::StorageConfig::default(),
            observability: systemprompt_manifest::profile::ObservabilityConfig::default(),
            config_planes: systemprompt_manifest::profile::ConfigPlanesConfig::default(),
            licence: systemprompt_manifest::profile::LicenceConfig::default(),
            notifications: systemprompt_manifest::profile::NotificationsProfile::default(),
            brain: systemprompt_manifest::profile::BrainProfile::default(),
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
                migrate_on_boot: true,
                pool: None,
            },
            security: security_config(self.trusted_issuers, false),
            server: {
                let mut server =
                    ServerConfig::new(consts::CLOUD_HOST, consts::DEFAULT_PORT, external);
                server.role = systemprompt_manifest::profile::NodeRole::All;
                server.trusted_proxies = trusted_proxies;
                server
            },
            paths: cloud_paths(self.geoip_database),
            rate_limits: RateLimitsConfig::default(),
            runtime: cloud_runtime_config(),
            cloud: Some(CloudConfig {
                tenant_id: self.tenant_id,
                provider: self.provider,
                validation: CloudValidationMode::Strict,
                deploy: None,
                operator: None,
            }),
            secrets: Some(SecretsConfig {
                secrets_path: self.secrets_path,
                validation: SecretsValidationMode::Strict,
                source: SecretsSource::Env,
                vault: None,
                external: None,
                refresh_secs: None,
            }),
            extensions: ExtensionsConfig::default(),
            governance: Some(webhook_governance()),
            services: systemprompt_manifest::profile::ServicesProfileConfig::default(),
            judge: systemprompt_manifest::profile::JudgeProfile::default(),
            retention: systemprompt_manifest::profile::RetentionConfig::default(),
            system_admin: SystemAdminConfig {
                username: "admin".to_owned(),
                email: Some(Email::local_admin()),
            },
        };
        profile.derive_fields()?;
        Ok(profile)
    }

    fn resolve_trusted_proxies(&self) -> ProfileResult<Vec<IpNet>> {
        match (&self.trusted_proxies, self.provider) {
            (Some(nets), provider) if !nets.is_empty() || provider.is_systemprompt() => {
                Ok(nets.clone())
            },
            (None, HostingProvider::Systemprompt) => Ok(
                crate::trusted_proxies::default_trusted_proxies(HostingProvider::Systemprompt),
            ),
            (_, provider) => Err(ProfileError::Validation {
                name: self.name.clone(),
                errors: vec![format!(
                    "server.trusted_proxies is required for a {provider} profile: list the \
                     ingress and load-balancer ranges in front of the pods"
                )],
            }),
        }
    }
}

fn cloud_paths(geoip_database: Option<String>) -> PathsConfig {
    PathsConfig {
        root: None,
        system: container::APP.to_owned(),
        services: container::SERVICES.to_owned(),
        bin: container::BIN.to_owned(),
        storage: Some(container::STORAGE.to_owned()),
        geoip_database,
        data: None,
        web_path: Some(container::WEB.to_owned()),
        shipped_files: None,
    }
}
