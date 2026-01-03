use anyhow::Result;
use systemprompt_cloud::ProjectContext;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{SecretsConfig, SecretsSource, SecretsValidationMode};
use systemprompt_models::{
    CloudConfig, CloudValidationMode, Environment, LogLevel, OutputFormat, PathsConfig, Profile,
    ProfileDatabaseConfig, ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig,
    ServerConfig, SiteConfig,
};

use super::templates::generate_display_name;

pub fn build_local_profile(
    name: &str,
    tenant_id: Option<String>,
    secrets_path: &str,
    services_path: &str,
) -> Result<Profile> {
    let ctx = ProjectContext::discover();
    let root = ctx.root();
    let system_path = root.to_string_lossy().to_string();
    let core_path = root.join("core").to_string_lossy().to_string();
    let display_name = generate_display_name(name);

    Ok(Profile {
        name: name.to_string(),
        display_name,
        target: ProfileType::Local,
        site: SiteConfig {
            name: "SystemPrompt".to_string(),
            github_link: None,
            service_display_name: Some("SystemPrompt".to_string()),
            service_version: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
        },
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_string(),
            api_internal_url: "http://localhost:8080".to_string(),
            api_external_url: "http://localhost:8080".to_string(),
            use_https: false,
            cors_allowed_origins: vec![
                "http://localhost:8080".to_string(),
                "http://localhost:5173".to_string(),
            ],
        },
        paths: PathsConfig {
            system: system_path.clone(),
            core: core_path.clone(),
            services: services_path.to_string(),
            skills: Some(format!("{}/skills", services_path)),
            config: Some(format!("{}/config/config.yaml", services_path)),
            storage: Some(ctx.storage_dir().to_string_lossy().to_string()),
            geoip_database: None,
            ai_config: Some(format!("{}/ai/config.yaml", services_path)),
            content_config: Some(format!("{}/content/config.yaml", services_path)),
            web_config: Some(format!("{}/web/config.yaml", services_path)),
            web_metadata: Some(format!("{}/web/metadata.yaml", services_path)),
            web_path: Some(format!("{}/web", core_path)),
        },
        security: SecurityConfig {
            issuer: "systemprompt-local".to_string(),
            access_token_expiration: 86400,
            refresh_token_expiration: 2_592_000,
            audiences: vec![
                JwtAudience::Web,
                JwtAudience::Api,
                JwtAudience::A2a,
                JwtAudience::Mcp,
            ],
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
            credentials_path: "../../credentials.json".to_string(),
            tenants_path: "../../tenants.json".to_string(),
            tenant_id,
            cli_enabled: true,
            validation: CloudValidationMode::Warn,
        }),
        extensions: None,
        secrets: Some(SecretsConfig {
            secrets_path: secrets_path.to_string(),
            validation: SecretsValidationMode::Warn,
            source: SecretsSource::File,
        }),
    })
}

pub fn build_cloud_profile(
    name: &str,
    tenant_id: Option<String>,
    _services_path: &str,
    external_url: Option<&str>,
    _secrets_path: &str,
) -> Result<Profile> {
    let display_name = generate_display_name(name);
    let external = external_url
        .map(String::from)
        .unwrap_or_else(|| "https://cloud.systemprompt.io".to_string());

    Ok(Profile {
        name: name.to_string(),
        display_name,
        target: ProfileType::Cloud,
        site: SiteConfig {
            name: "SystemPrompt".to_string(),
            github_link: None,
            service_display_name: Some("SystemPrompt".to_string()),
            service_version: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
        },
        server: ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            api_server_url: external.clone(),
            api_internal_url: external.clone(),
            api_external_url: external.clone(),
            use_https: true,
            cors_allowed_origins: vec![external],
        },
        paths: PathsConfig {
            system: "/app".to_string(),
            core: "/app".to_string(),
            services: "/app/services".to_string(),
            skills: Some("/app/services/skills".to_string()),
            config: Some("/app/services/config/config.yaml".to_string()),
            storage: Some("/app/storage".to_string()),
            geoip_database: None,
            ai_config: Some("/app/services/ai/config.yaml".to_string()),
            content_config: Some("/app/services/content/config.yaml".to_string()),
            web_config: Some("/app/services/web/config.yaml".to_string()),
            web_metadata: Some("/app/services/web/metadata.yaml".to_string()),
            web_path: Some("/app/web".to_string()),
        },
        security: SecurityConfig {
            issuer: "systemprompt".to_string(),
            access_token_expiration: 86400,
            refresh_token_expiration: 2_592_000,
            audiences: vec![
                JwtAudience::Web,
                JwtAudience::Api,
                JwtAudience::A2a,
                JwtAudience::Mcp,
            ],
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
            credentials_path: String::new(),
            tenants_path: String::new(),
            tenant_id,
            cli_enabled: false,
            validation: CloudValidationMode::Strict,
        }),
        extensions: None,
        secrets: Some(SecretsConfig {
            secrets_path: String::new(),
            validation: SecretsValidationMode::Strict,
            source: SecretsSource::Env,
        }),
    })
}
