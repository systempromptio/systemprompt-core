use super::{
    ContentNegotiationConfig, DatabaseConfig, ExtensionsConfig, PathsConfig, Profile, ProfileError,
    ProfileResult, ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig,
    SecurityHeadersConfig, ServerConfig, SiteConfig, TierMultipliers, default_agent_registry,
    default_agents, default_artifacts, default_burst, default_content, default_contexts,
    default_mcp, default_mcp_registry, default_oauth_auth, default_oauth_public, default_stream,
    default_tasks,
};

impl Profile {
    pub fn from_env(profile_name: &str, display_name: &str) -> ProfileResult<Self> {
        let db_type = require_env("DATABASE_TYPE")?;

        Ok(Self {
            name: profile_name.to_string(),
            display_name: display_name.to_string(),
            target: ProfileType::Cloud,
            site: site_config_from_env()?,
            database: DatabaseConfig {
                db_type,
                external_db_access: false,
            },
            server: server_config_from_env()?,
            paths: paths_config_from_env()?,
            security: security_config_from_env()?,
            rate_limits: rate_limits_from_env(),
            runtime: runtime_config_from_env()?,
            cloud: None,
            secrets: None,
            extensions: ExtensionsConfig::default(),
            gateway: None,
        })
    }
}

fn get_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn require_env(name: &'static str) -> ProfileResult<String> {
    std::env::var(name).map_err(|_| ProfileError::MissingEnvVar { name })
}

fn site_config_from_env() -> ProfileResult<SiteConfig> {
    Ok(SiteConfig {
        name: require_env("SITENAME")?,
        github_link: get_env("GITHUB_LINK"),
    })
}

fn server_config_from_env() -> ProfileResult<ServerConfig> {
    let port = require_env("PORT")?
        .parse()
        .map_err(|e: std::num::ParseIntError| ProfileError::InvalidEnvVar {
            name: "PORT",
            message: e.to_string(),
        })?;

    Ok(ServerConfig {
        host: require_env("HOST")?,
        port,
        api_server_url: require_env("API_SERVER_URL")?,
        api_internal_url: require_env("API_INTERNAL_URL")?,
        api_external_url: require_env("API_EXTERNAL_URL")?,
        use_https: get_env("USE_HTTPS").is_some_and(|v| v.to_lowercase() == "true"),
        cors_allowed_origins: get_env("CORS_ALLOWED_ORIGINS").map_or_else(Vec::new, |s| {
            s.split(',').map(|s| s.trim().to_string()).collect()
        }),
        content_negotiation: ContentNegotiationConfig {
            enabled: get_env("CONTENT_NEGOTIATION_ENABLED")
                .is_some_and(|v| v.to_lowercase() == "true"),
            ..Default::default()
        },
        security_headers: SecurityHeadersConfig::default(),
    })
}

fn paths_config_from_env() -> ProfileResult<PathsConfig> {
    Ok(PathsConfig {
        system: require_env("SYSTEM_PATH")?,
        services: require_env("SYSTEMPROMPT_SERVICES_PATH")?,
        bin: require_env("BIN_PATH")?,
        storage: get_env("STORAGE_PATH"),
        geoip_database: get_env("GEOIP_DATABASE_PATH"),
        web_path: get_env("SYSTEMPROMPT_WEB_PATH"),
    })
}

fn security_config_from_env() -> ProfileResult<SecurityConfig> {
    use crate::auth::JwtAudience;

    let issuer = require_env("JWT_ISSUER")?;

    let access_token_expiration = require_env("JWT_ACCESS_TOKEN_EXPIRATION")?
        .parse()
        .map_err(|e: std::num::ParseIntError| ProfileError::InvalidEnvVar {
            name: "JWT_ACCESS_TOKEN_EXPIRATION",
            message: e.to_string(),
        })?;

    let refresh_token_expiration = require_env("JWT_REFRESH_TOKEN_EXPIRATION")?
        .parse()
        .map_err(|e: std::num::ParseIntError| ProfileError::InvalidEnvVar {
            name: "JWT_REFRESH_TOKEN_EXPIRATION",
            message: e.to_string(),
        })?;

    let audiences_raw = require_env("JWT_AUDIENCES")?;
    let audiences = audiences_raw
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<JwtAudience>()
                .map_err(|e| ProfileError::InvalidEnvVar {
                    name: "JWT_AUDIENCES",
                    message: e.to_string(),
                })
        })
        .collect::<ProfileResult<Vec<_>>>()?;

    let allow_registration =
        get_env("ALLOW_REGISTRATION").is_none_or(|s| s.eq_ignore_ascii_case("true"));

    Ok(SecurityConfig {
        issuer,
        access_token_expiration,
        refresh_token_expiration,
        audiences,
        allow_registration,
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

fn runtime_config_from_env() -> ProfileResult<RuntimeConfig> {
    let environment = get_env("SYSTEMPROMPT_ENV")
        .unwrap_or_else(|| "development".to_string())
        .parse()
        .map_err(|e: String| ProfileError::InvalidEnvVar {
            name: "SYSTEMPROMPT_ENV",
            message: e,
        })?;

    let log_level = get_env("SYSTEMPROMPT_LOG_LEVEL")
        .unwrap_or_else(|| "normal".to_string())
        .parse()
        .map_err(|e: String| ProfileError::InvalidEnvVar {
            name: "SYSTEMPROMPT_LOG_LEVEL",
            message: e,
        })?;

    let output_format = get_env("SYSTEMPROMPT_OUTPUT_FORMAT")
        .unwrap_or_else(|| "text".to_string())
        .parse()
        .map_err(|e: String| ProfileError::InvalidEnvVar {
            name: "SYSTEMPROMPT_OUTPUT_FORMAT",
            message: e,
        })?;

    Ok(RuntimeConfig {
        environment,
        log_level,
        output_format,
        no_color: get_env("NO_COLOR").is_some(),
        non_interactive: get_env("CI").is_some(),
    })
}
