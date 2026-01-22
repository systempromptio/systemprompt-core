//! Profile creation from environment variables.
//!
//! This module provides functionality to create Profile configurations
//! from environment variables, typically used in cloud/container deployments.

use super::{
    default_agent_registry, default_agents, default_artifacts, default_burst, default_content,
    default_contexts, default_mcp, default_mcp_registry, default_oauth_auth, default_oauth_public,
    default_stream, default_tasks, DatabaseConfig, ExtensionsConfig, PathsConfig, Profile,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, ServerConfig, SiteConfig,
    TierMultipliers,
};
use anyhow::{Context, Result};

impl Profile {
    /// Creates a Profile from environment variables.
    ///
    /// This is primarily used for cloud deployments where configuration
    /// is passed via environment variables rather than files.
    pub fn from_env(profile_name: &str, display_name: &str) -> Result<Self> {
        let require_env = |key: &str| -> Result<String> {
            std::env::var(key)
                .with_context(|| format!("Missing required environment variable: {}", key))
        };

        let db_type = get_env("DATABASE_TYPE")
            .ok_or_else(|| anyhow::anyhow!("DATABASE_TYPE environment variable is required"))?;

        Ok(Self {
            name: profile_name.to_string(),
            display_name: display_name.to_string(),
            target: ProfileType::Cloud,
            site: site_config_from_env(&require_env)?,
            database: DatabaseConfig {
                db_type,
                external_db_access: false,
            },
            server: server_config_from_env(&require_env)?,
            paths: paths_config_from_env(&require_env)?,
            security: security_config_from_env()?,
            rate_limits: rate_limits_from_env(),
            runtime: runtime_config_from_env()?,
            cloud: None,
            secrets: None,
            extensions: ExtensionsConfig::default(),
        })
    }
}

fn get_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn site_config_from_env(require_env: &dyn Fn(&str) -> Result<String>) -> Result<SiteConfig> {
    Ok(SiteConfig {
        name: require_env("SITENAME")?,
        github_link: get_env("GITHUB_LINK"),
    })
}

fn server_config_from_env(require_env: &dyn Fn(&str) -> Result<String>) -> Result<ServerConfig> {
    Ok(ServerConfig {
        host: require_env("HOST")?,
        port: require_env("PORT")?.parse().context("Invalid PORT")?,
        api_server_url: require_env("API_SERVER_URL")?,
        api_internal_url: require_env("API_INTERNAL_URL")?,
        api_external_url: require_env("API_EXTERNAL_URL")?,
        use_https: get_env("USE_HTTPS").is_some_and(|v| v.to_lowercase() == "true"),
        cors_allowed_origins: get_env("CORS_ALLOWED_ORIGINS")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
    })
}

fn paths_config_from_env(require_env: &dyn Fn(&str) -> Result<String>) -> Result<PathsConfig> {
    Ok(PathsConfig {
        system: require_env("SYSTEM_PATH")?,
        services: require_env("SYSTEMPROMPT_SERVICES_PATH")?,
        bin: require_env("BIN_PATH")?,
        storage: get_env("STORAGE_PATH"),
        geoip_database: get_env("GEOIP_DATABASE_PATH"),
        web_path: get_env("SYSTEMPROMPT_WEB_PATH"),
    })
}

fn security_config_from_env() -> Result<SecurityConfig> {
    use crate::auth::JwtAudience;

    let issuer = get_env("JWT_ISSUER")
        .ok_or_else(|| anyhow::anyhow!("JWT_ISSUER environment variable is required"))?;

    let access_token_expiration = get_env("JWT_ACCESS_TOKEN_EXPIRATION")
        .ok_or_else(|| {
            anyhow::anyhow!("JWT_ACCESS_TOKEN_EXPIRATION environment variable is required")
        })?
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse JWT_ACCESS_TOKEN_EXPIRATION: {e}"))?;

    let refresh_token_expiration = get_env("JWT_REFRESH_TOKEN_EXPIRATION")
        .ok_or_else(|| {
            anyhow::anyhow!("JWT_REFRESH_TOKEN_EXPIRATION environment variable is required")
        })?
        .parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse JWT_REFRESH_TOKEN_EXPIRATION: {e}"))?;

    let audiences = get_env("JWT_AUDIENCES")
        .ok_or_else(|| anyhow::anyhow!("JWT_AUDIENCES environment variable is required"))?
        .split(',')
        .map(|s| s.trim().parse::<JwtAudience>())
        .collect::<Result<Vec<_>>>()?;

    Ok(SecurityConfig {
        issuer,
        access_token_expiration,
        refresh_token_expiration,
        audiences,
    })
}

fn rate_limits_from_env() -> RateLimitsConfig {
    let parse_rate = |key: &str, default: fn() -> u64| -> u64 {
        get_env(key)
            .and_then(|s| {
                s.parse()
                    .map_err(|e| {
                        tracing::warn!(key = %key, value = %s, error = %e, "Failed to parse rate limit value");
                        e
                    })
                    .ok()
            })
            .unwrap_or_else(default)
    };

    RateLimitsConfig {
        disabled: get_env("RATE_LIMIT_DISABLED").is_some_and(|v| v.to_lowercase() == "true"),
        oauth_public_per_second: parse_rate(
            "RATE_LIMIT_OAUTH_PUBLIC_PER_SECOND",
            default_oauth_public,
        ),
        oauth_auth_per_second: parse_rate("RATE_LIMIT_OAUTH_AUTH_PER_SECOND", default_oauth_auth),
        contexts_per_second: parse_rate("RATE_LIMIT_CONTEXTS_PER_SECOND", default_contexts),
        tasks_per_second: parse_rate("RATE_LIMIT_TASKS_PER_SECOND", default_tasks),
        artifacts_per_second: parse_rate("RATE_LIMIT_ARTIFACTS_PER_SECOND", default_artifacts),
        agent_registry_per_second: parse_rate(
            "RATE_LIMIT_AGENT_REGISTRY_PER_SECOND",
            default_agent_registry,
        ),
        agents_per_second: parse_rate("RATE_LIMIT_AGENTS_PER_SECOND", default_agents),
        mcp_registry_per_second: parse_rate(
            "RATE_LIMIT_MCP_REGISTRY_PER_SECOND",
            default_mcp_registry,
        ),
        mcp_per_second: parse_rate("RATE_LIMIT_MCP_PER_SECOND", default_mcp),
        stream_per_second: parse_rate("RATE_LIMIT_STREAM_PER_SECOND", default_stream),
        content_per_second: parse_rate("RATE_LIMIT_CONTENT_PER_SECOND", default_content),
        burst_multiplier: parse_rate("RATE_LIMIT_BURST_MULTIPLIER", default_burst),
        tier_multipliers: TierMultipliers::default(),
    }
}

fn runtime_config_from_env() -> Result<RuntimeConfig> {
    let parse_or_default = |key: &str, default: &str| -> Result<String> {
        Ok(get_env(key).unwrap_or_else(|| default.to_string()))
    };

    Ok(RuntimeConfig {
        environment: parse_or_default("SYSTEMPROMPT_ENV", "development")?
            .parse()
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        log_level: parse_or_default("SYSTEMPROMPT_LOG_LEVEL", "normal")?
            .parse()
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        output_format: parse_or_default("SYSTEMPROMPT_OUTPUT_FORMAT", "text")?
            .parse()
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        no_color: get_env("NO_COLOR").is_some(),
        non_interactive: get_env("CI").is_some(),
    })
}
