use anyhow::Result;
use systemprompt_cloud::ProjectContext;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{SecretsConfig, SecretsSource, SecretsValidationMode};
use systemprompt_models::{
    CloudConfig, CloudValidationMode, Environment, LogLevel, OutputFormat, PathsConfig, Profile,
    ProfileDatabaseConfig, RateLimitsConfig, RuntimeConfig, SecurityConfig, ServerConfig,
    SiteConfig,
};

pub fn build_local_profile(
    name: &str,
    display_name: &str,
    tenant_id: Option<String>,
    secrets_path: &str,
    services_path: &str,
) -> Result<Profile> {
    let ctx = ProjectContext::discover();
    let root = ctx.root();
    let system_path = root.to_string_lossy().to_string();
    let core_path = root.join("core").to_string_lossy().to_string();

    Ok(Profile {
        name: name.to_string(),
        display_name: display_name.to_string(),
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
            cargo_target: Some(format!("{}/target", core_path)),
            binary_dir: None,
            geoip_database: None,
            ai_config: Some(format!("{}/ai/config.yaml", services_path)),
            content_config: Some(format!("{}/content/config.yaml", services_path)),
            web_config: Some(format!("{}/web/config.yaml", services_path)),
            web_metadata: Some(format!("{}/web/metadata.yaml", services_path)),
            web_path: Some(format!("{}/web", core_path)),
            scg_templates: None,
            scg_assets: None,
            dockerfile: Some(ctx.dockerfile().to_string_lossy().to_string()),
            web_dist: Some(format!("{}/web/dist", core_path)),
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
            // Relative path from profile directory (.systemprompt/profiles/<name>/)
            // to credentials at .systemprompt/credentials.json
            credentials_path: "../../credentials.json".to_string(),
            tenants_path: "../../tenants.json".to_string(),
            tenant_id,
            enabled: true,
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
    display_name: &str,
    tenant_id: Option<String>,
    services_path: &str,
    external_url: Option<&str>,
) -> Result<Profile> {
    let ctx = ProjectContext::discover();
    let root = ctx.root();
    let system_path = root.to_string_lossy().to_string();
    let core_path = root.join("core").to_string_lossy().to_string();
    let external = external_url
        .map(String::from)
        .unwrap_or_else(|| "https://cloud.systemprompt.io".to_string());

    Ok(Profile {
        name: name.to_string(),
        display_name: display_name.to_string(),
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
            api_server_url: "http://localhost:8080".to_string(),
            api_internal_url: "http://localhost:8080".to_string(),
            api_external_url: external,
            use_https: true,
            cors_allowed_origins: vec!["*".to_string()],
        },
        paths: PathsConfig {
            system: system_path.clone(),
            core: core_path.clone(),
            services: services_path.to_string(),
            skills: Some(format!("{}/skills", services_path)),
            config: Some(format!("{}/config/config.yaml", services_path)),
            storage: Some(ctx.storage_dir().to_string_lossy().to_string()),
            cargo_target: Some(format!("{}/target", core_path)),
            binary_dir: None,
            geoip_database: None,
            ai_config: Some(format!("{}/ai/config.yaml", services_path)),
            content_config: Some(format!("{}/content/config.yaml", services_path)),
            web_config: Some(format!("{}/web/config.yaml", services_path)),
            web_metadata: Some(format!("{}/web/metadata.yaml", services_path)),
            web_path: Some(format!("{}/web", core_path)),
            scg_templates: None,
            scg_assets: None,
            dockerfile: Some(ctx.dockerfile().to_string_lossy().to_string()),
            web_dist: Some(format!("{}/web/dist", core_path)),
        },
        security: SecurityConfig {
            issuer: "systemprompt-cloud".to_string(),
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
            output_format: OutputFormat::Text,
            no_color: false,
            non_interactive: true,
        },
        cloud: Some(CloudConfig {
            // Cloud profiles use home directory paths (~ expansion)
            credentials_path: "~/.systemprompt/credentials.json".to_string(),
            tenants_path: "~/.systemprompt/tenants.json".to_string(),
            tenant_id,
            enabled: true,
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
