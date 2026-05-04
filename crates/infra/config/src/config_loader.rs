use anyhow::Result;

use systemprompt_models::Config;
use systemprompt_models::config::{
    format_path_errors, validate_postgres_url, validate_profile_paths,
};
use systemprompt_models::profile::Profile;

use crate::bootstrap::{ProfileBootstrap, SecretsBootstrap};

struct BuildConfigPaths {
    system: String,
    skills: String,
    settings: String,
    content_config: String,
    web: String,
    web_config: String,
    web_metadata: String,
}

pub fn init_config() -> Result<()> {
    let profile =
        ProfileBootstrap::get().map_err(|e| anyhow::anyhow!("Profile not initialized: {}", e))?;

    let config = build_from_profile(profile)?;
    Config::install(config).map_err(|_| anyhow::anyhow!("Config already initialized"))?;
    Ok(())
}

pub fn try_init_config() -> Result<()> {
    if Config::is_initialized() {
        return Ok(());
    }
    init_config()
}

pub fn build_from_profile(profile: &Profile) -> Result<Config> {
    let profile_path =
        ProfileBootstrap::get_path().map_or_else(|_| "<not set>".to_string(), ToString::to_string);

    let path_report = validate_profile_paths(profile, &profile_path);
    if path_report.has_errors() {
        return Err(anyhow::anyhow!(
            "{}",
            format_path_errors(&path_report, &profile_path)
        ));
    }

    let system_path = canonicalize_path(&profile.paths.system, "system")?;

    let skills_path = profile.paths.skills();
    let settings_path = require_yaml_path("config", Some(&profile.paths.config()), &profile_path)?;
    let content_config_path = require_yaml_path(
        "content_config",
        Some(&profile.paths.content_config()),
        &profile_path,
    )?;
    let web_path = profile.paths.web_path_resolved();
    let web_config_path = require_yaml_path(
        "web_config",
        Some(&profile.paths.web_config()),
        &profile_path,
    )?;
    let web_metadata_path = require_yaml_path(
        "web_metadata",
        Some(&profile.paths.web_metadata()),
        &profile_path,
    )?;

    let paths = BuildConfigPaths {
        system: system_path,
        skills: skills_path,
        settings: settings_path,
        content_config: content_config_path,
        web: web_path,
        web_config: web_config_path,
        web_metadata: web_metadata_path,
    };
    let config = build_config(profile, paths)?;

    validate_database_config(&config)?;
    Ok(config)
}

pub fn init_config_from_profile(profile: &Profile) -> Result<()> {
    let config = build_from_profile(profile)?;
    Config::install(config).map_err(|_| anyhow::anyhow!("Config already initialized"))?;
    Ok(())
}

fn canonicalize_path(path: &str, name: &str) -> Result<String> {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| anyhow::anyhow!("Failed to canonicalize {} path: {}", name, e))
}

fn require_yaml_path(field: &str, value: Option<&str>, profile_path: &str) -> Result<String> {
    let path = value.ok_or_else(|| anyhow::anyhow!("Missing required path: paths.{}", field))?;

    let content = std::fs::read_to_string(path).map_err(|e| {
        anyhow::anyhow!(
            "Profile Error: Cannot read file\n\n  Field: paths.{}\n  Path: {}\n  Error: {}\n  \
             Profile: {}",
            field,
            path,
            e,
            profile_path
        )
    })?;

    serde_yaml::from_str::<serde_yaml::Value>(&content).map_err(|e| {
        anyhow::anyhow!(
            "Profile Error: Invalid YAML syntax\n\n  Field: paths.{}\n  Path: {}\n  Error: {}\n  \
             Profile: {}",
            field,
            path,
            e,
            profile_path
        )
    })?;

    Ok(path.to_string())
}

fn build_config(profile: &Profile, paths: BuildConfigPaths) -> Result<Config> {
    let secrets = SecretsBootstrap::get().map_err(|_| {
        anyhow::anyhow!(
            "Secrets not initialized. Call SecretsBootstrap::init() before Config::from_profile()"
        )
    })?;

    Ok(Config {
        sitename: profile.site.name.clone(),
        database_type: profile.database.db_type.clone(),
        database_url: secrets.database_url.clone(),
        database_write_url: secrets.database_write_url.clone(),
        github_link: profile
            .site
            .github_link
            .clone()
            .unwrap_or_else(|| "https://github.com/systemprompt/systemprompt-os".to_string()),
        github_token: secrets.github.clone(),
        system_path: paths.system,
        services_path: profile.paths.services.clone(),
        bin_path: profile.paths.bin.clone(),
        skills_path: paths.skills,
        settings_path: paths.settings,
        content_config_path: paths.content_config,
        geoip_database_path: profile.paths.geoip_database.clone(),
        web_path: paths.web,
        web_config_path: paths.web_config,
        web_metadata_path: paths.web_metadata,
        host: profile.server.host.clone(),
        port: profile.server.port,
        api_server_url: profile.server.api_server_url.clone(),
        api_internal_url: profile.server.api_internal_url.clone(),
        api_external_url: profile.server.api_external_url.clone(),
        jwt_issuer: profile.security.issuer.clone(),
        jwt_access_token_expiration: profile.security.access_token_expiration,
        jwt_refresh_token_expiration: profile.security.refresh_token_expiration,
        jwt_audiences: profile.security.audiences.clone(),
        use_https: profile.server.use_https,
        rate_limits: (&profile.rate_limits).into(),
        cors_allowed_origins: profile.server.cors_allowed_origins.clone(),
        is_cloud: profile.target.is_cloud(),
        content_negotiation: profile.server.content_negotiation.clone(),
        security_headers: profile.server.security_headers.clone(),
        allow_registration: profile.security.allow_registration,
    })
}

pub fn validate_database_config(config: &Config) -> Result<()> {
    let db_type = config.database_type.to_lowercase();

    if db_type != "postgres" && db_type != "postgresql" {
        return Err(anyhow::anyhow!(
            "Unsupported database type '{}'. Only 'postgres' is supported.",
            config.database_type
        ));
    }

    validate_postgres_url(&config.database_url)?;
    if let Some(write_url) = &config.database_write_url {
        validate_postgres_url(write_url)?;
    }
    Ok(())
}
