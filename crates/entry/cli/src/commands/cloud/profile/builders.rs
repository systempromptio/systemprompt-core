use std::path::Path;

use systemprompt_cloud::constants::{container, profile as consts};
use systemprompt_cloud::ProjectContext;
use systemprompt_identifiers::TenantId;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{SecretsConfig, SecretsSource, SecretsValidationMode};
use systemprompt_models::{
    CloudConfig, CloudValidationMode, ContentNegotiationConfig, Environment, ExtensionsConfig,
    LogLevel, OutputFormat, PathsConfig, Profile, ProfileDatabaseConfig, ProfileType,
    RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig, ServerConfig,
    SiteConfig,
};

use super::templates::generate_display_name;

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

    pub fn with_tenant_id(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn build(self) -> Profile {
        let ctx = ProjectContext::discover();
        let root = ctx.root();
        let system_path = root.to_string_lossy().to_string();
        let display_name = generate_display_name(&self.name);
        let local_url = format!("http://localhost:{}", consts::DEFAULT_PORT);

        Profile {
            name: self.name,
            display_name,
            target: ProfileType::Local,
            site: SiteConfig {
                name: "systemprompt.io".to_string(),
                github_link: None,
            },
            database: ProfileDatabaseConfig {
                db_type: consts::DEFAULT_DB_TYPE.to_string(),
                external_db_access: false,
            },
            server: ServerConfig {
                host: consts::LOCAL_HOST.to_string(),
                port: consts::DEFAULT_PORT,
                api_server_url: local_url.clone(),
                api_internal_url: local_url.clone(),
                api_external_url: local_url.clone(),
                use_https: false,
                cors_allowed_origins: vec![local_url, "http://localhost:5173".to_string()],
                content_negotiation: ContentNegotiationConfig::default(),
                security_headers: SecurityHeadersConfig::default(),
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
            security: SecurityConfig {
                issuer: consts::LOCAL_ISSUER.to_string(),
                access_token_expiration: consts::ACCESS_TOKEN_EXPIRATION,
                refresh_token_expiration: consts::REFRESH_TOKEN_EXPIRATION,
                audiences: vec![
                    JwtAudience::Web,
                    JwtAudience::Api,
                    JwtAudience::A2a,
                    JwtAudience::Mcp,
                ],
                allow_registration: true,
            },
            rate_limits: RateLimitsConfig {
                disabled: true,
                ..Default::default()
            },
            runtime: RuntimeConfig {
                environment: Environment::Development,
                log_level: LogLevel::Verbose,
                output_format: OutputFormat::Text,
                no_color: false,
                non_interactive: false,
            },
            cloud: Some(CloudConfig {
                tenant_id: self.tenant_id.map(|id| id.to_string()),
                validation: CloudValidationMode::Warn,
            }),
            secrets: Some(SecretsConfig {
                secrets_path: self.secrets_path,
                validation: SecretsValidationMode::Warn,
                source: SecretsSource::File,
            }),
            extensions: ExtensionsConfig::default(),
        }
    }
}

pub struct CloudProfileBuilder {
    name: String,
    tenant_id: Option<TenantId>,
    external_url: Option<String>,
    external_db_access: bool,
    secrets_path: Option<String>,
}

impl CloudProfileBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tenant_id: None,
            external_url: None,
            external_db_access: false,
            secrets_path: None,
        }
    }

    pub fn with_tenant_id(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn with_external_url(mut self, url: impl Into<String>) -> Self {
        self.external_url = Some(url.into());
        self
    }

    pub const fn with_external_db_access(mut self, enabled: bool) -> Self {
        self.external_db_access = enabled;
        self
    }

    pub fn with_secrets_path(mut self, path: impl Into<String>) -> Self {
        self.secrets_path = Some(path.into());
        self
    }

    pub fn build(self) -> Profile {
        let display_name = generate_display_name(&self.name);
        let external = self
            .external_url
            .unwrap_or_else(|| consts::DEFAULT_CLOUD_URL.to_string());
        let internal_url = format!("http://localhost:{}", consts::DEFAULT_PORT);

        Profile {
            name: self.name,
            display_name,
            target: ProfileType::Cloud,
            site: SiteConfig {
                name: "systemprompt.io".to_string(),
                github_link: None,
            },
            database: ProfileDatabaseConfig {
                db_type: consts::DEFAULT_DB_TYPE.to_string(),
                external_db_access: self.external_db_access,
            },
            server: ServerConfig {
                host: consts::CLOUD_HOST.to_string(),
                port: consts::DEFAULT_PORT,
                api_server_url: external.clone(),
                api_internal_url: internal_url,
                api_external_url: external.clone(),
                use_https: true,
                cors_allowed_origins: vec![external],
                content_negotiation: ContentNegotiationConfig::default(),
                security_headers: SecurityHeadersConfig::default(),
            },
            paths: PathsConfig {
                system: container::APP.to_string(),
                services: container::SERVICES.to_string(),
                bin: container::BIN.to_string(),
                storage: Some(container::STORAGE.to_string()),
                geoip_database: None,
                web_path: Some(container::WEB.to_string()),
            },
            security: SecurityConfig {
                issuer: consts::CLOUD_ISSUER.to_string(),
                access_token_expiration: consts::ACCESS_TOKEN_EXPIRATION,
                refresh_token_expiration: consts::REFRESH_TOKEN_EXPIRATION,
                audiences: vec![
                    JwtAudience::Web,
                    JwtAudience::Api,
                    JwtAudience::A2a,
                    JwtAudience::Mcp,
                ],
                allow_registration: true,
            },
            rate_limits: RateLimitsConfig::default(),
            runtime: RuntimeConfig {
                environment: Environment::Production,
                log_level: LogLevel::Normal,
                output_format: OutputFormat::Json,
                no_color: true,
                non_interactive: true,
            },
            cloud: Some(CloudConfig {
                tenant_id: self.tenant_id.map(|id| id.to_string()),
                validation: CloudValidationMode::Strict,
            }),
            secrets: Some(SecretsConfig {
                secrets_path: self.secrets_path.unwrap_or_else(String::new),
                validation: SecretsValidationMode::Strict,
                source: SecretsSource::Env,
            }),
            extensions: ExtensionsConfig::default(),
        }
    }
}
