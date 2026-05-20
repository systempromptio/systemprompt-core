//! Build a [`Config`] from a validated profile and install it into
//! the global cell.
//!
//! These functions are the bridge from the bootstrap layer (profile +
//! secrets) into the runtime [`Config`] singleton consumed by the rest
//! of the application.

use std::path::{Path, PathBuf};

use systemprompt_models::Config;
use systemprompt_models::config::{
    format_path_errors, validate_postgres_url, validate_profile_paths,
};
use systemprompt_models::profile::Profile;

use crate::bootstrap::{ProfileBootstrap, SecretsBootstrap};
use crate::error::{ConfigError, ConfigResult};

struct BuildConfigPaths {
    system: String,
    skills: String,
    settings: String,
    content_config: String,
    web: String,
    web_config: String,
    web_metadata: String,
}

pub fn init_config() -> ConfigResult<()> {
    let profile = ProfileBootstrap::get()?;
    let config = build_from_profile(profile)?;
    Config::install(config).map_err(|_| ConfigError::AlreadyInitialized)?;
    Ok(())
}

pub fn try_init_config() -> ConfigResult<()> {
    if Config::is_initialized() {
        return Ok(());
    }
    init_config()
}

pub fn build_from_profile(profile: &Profile) -> ConfigResult<Config> {
    let profile_path =
        ProfileBootstrap::get_path().map_or_else(|_| "<not set>".to_string(), ToString::to_string);

    let path_report = validate_profile_paths(profile, &profile_path);
    if path_report.has_errors() {
        return Err(ConfigError::ProfilePathReport {
            message: format_path_errors(&path_report, &profile_path),
        });
    }

    let system_path = canonicalize_path(&profile.paths.system, "system")?;

    let skills_path = profile.paths.skills();
    let settings_path = require_yaml_path("config", Some(&profile.paths.config()))?;
    let content_config_path =
        require_yaml_path("content_config", Some(&profile.paths.content_config()))?;
    let web_path = profile.paths.web_path_resolved();
    let web_config_path = require_yaml_path("web_config", Some(&profile.paths.web_config()))?;
    let web_metadata_path = require_yaml_path("web_metadata", Some(&profile.paths.web_metadata()))?;

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

pub fn init_config_from_profile(profile: &Profile) -> ConfigResult<()> {
    let config = build_from_profile(profile)?;
    Config::install(config).map_err(|_| ConfigError::AlreadyInitialized)?;
    Ok(())
}

fn canonicalize_path(path: &str, name: &str) -> ConfigResult<String> {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|source| ConfigError::CanonicalizePath {
            name: name.to_string(),
            source,
        })
}

fn require_yaml_path(field: &str, value: Option<&str>) -> ConfigResult<String> {
    let path = value.ok_or_else(|| ConfigError::MissingProfilePath {
        field: field.to_string(),
    })?;

    let content = std::fs::read_to_string(path).map_err(|source| ConfigError::ReadProfilePath {
        field: field.to_string(),
        path: PathBuf::from(path),
        source,
    })?;

    serde_yaml::from_str::<serde_yaml::Value>(&content).map_err(|source| {
        ConfigError::InvalidProfileYaml {
            field: field.to_string(),
            path: PathBuf::from(path),
            source,
        }
    })?;

    Ok(path.to_string())
}

fn build_config(profile: &Profile, paths: BuildConfigPaths) -> ConfigResult<Config> {
    let secrets = SecretsBootstrap::get()?;
    let system_admin_username = resolve_system_admin_username(profile)?;

    Ok(Config {
        instance_id: profile
            .server
            .instance_id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(systemprompt_models::config::default_instance_id),
        max_concurrent_streams: profile.server.max_concurrent_streams,
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
        system_path: paths.system.clone(),
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
        allowed_resource_audiences: profile.security.allowed_resource_audiences.clone(),
        trusted_issuers: profile.security.trusted_issuers.clone(),
        signing_key_path: resolve_signing_key_path(
            &profile.security.signing_key_path,
            &paths.system,
        ),
        use_https: profile.server.use_https,
        rate_limits: (&profile.rate_limits).into(),
        cors_allowed_origins: profile.server.cors_allowed_origins.clone(),
        is_cloud: profile.target.is_cloud(),
        content_negotiation: profile.server.content_negotiation.clone(),
        security_headers: profile.server.security_headers.clone(),
        allow_registration: profile.security.allow_registration,
        system_admin_username,
    })
}

fn resolve_system_admin_username(profile: &Profile) -> ConfigResult<String> {
    let env_override = std::env::var("SYSTEMPROMPT_SYSTEM_ADMIN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(name) = env_override {
        return Ok(name);
    }
    let profile_value = profile.system_admin.username.trim();
    if profile_value.is_empty() {
        return Err(ConfigError::MissingSystemAdmin);
    }
    Ok(profile_value.to_string())
}

fn resolve_signing_key_path(profile_path: &Path, system_path: &str) -> PathBuf {
    if profile_path.is_absolute() {
        return profile_path.to_path_buf();
    }
    if system_path.is_empty() {
        return profile_path.to_path_buf();
    }
    PathBuf::from(system_path).join(profile_path)
}

pub fn validate_database_config(config: &Config) -> ConfigResult<()> {
    let db_type = config.database_type.to_lowercase();

    if db_type != "postgres" && db_type != "postgresql" {
        return Err(ConfigError::UnsupportedDatabaseType {
            db_type: config.database_type.clone(),
        });
    }

    validate_postgres_url(&config.database_url).map_err(|e| ConfigError::InvalidDatabaseUrl {
        message: e.to_string(),
    })?;
    if let Some(write_url) = &config.database_write_url {
        validate_postgres_url(write_url).map_err(|e| ConfigError::InvalidDatabaseUrl {
            message: e.to_string(),
        })?;
    }
    Ok(())
}
