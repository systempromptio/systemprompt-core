//! Typed section builders for the generated setup profile.
//!
//! Split out so [`super::profile::build`] reads as a thin orchestration over
//! well-named sections. Each builder returns a fully-typed profile struct; the
//! governance section gives `admin setup` a complete, bootable profile. The
//! provider catalog and gateway routes are seeded into the services tree by
//! [`super::services_files`], not written here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_cloud::ProjectContext;
use systemprompt_loader::ExtensionLoader;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{
    AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, default_resource_audiences,
};
use systemprompt_models::{
    ContentNegotiationConfig, Environment, LogLevel, OutputFormat, PathsConfig, RuntimeConfig,
    SecurityConfig, SecurityHeadersConfig, ServerConfig,
};


pub(super) fn server(is_prod: bool) -> ServerConfig {
    ServerConfig {
        host: if is_prod {
            "0.0.0.0".to_owned()
        } else {
            "127.0.0.1".to_owned()
        },
        port: 8080,
        api_server_url: "http://localhost:8080".to_owned(),
        api_internal_url: "http://localhost:8080".to_owned(),
        api_external_url: "http://localhost:8080".to_owned(),
        use_https: is_prod,
        cors_allowed_origins: vec![
            "http://localhost:8080".to_owned(),
            "http://localhost:5173".to_owned(),
            "http://127.0.0.1:8080".to_owned(),
        ],
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        instance_id: None,
        metrics_port: None,
        max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
        trusted_proxies: Vec::new(),
    }
}

pub(super) fn paths(
    project_root: &Path,
    bin_path: Option<&Path>,
    ctx: &ProjectContext,
) -> PathsConfig {
    PathsConfig {
        system: project_root.to_string_lossy().to_string(),
        services: project_root.join("services").to_string_lossy().to_string(),
        bin: bin_path.map_or_else(
            || {
                ExtensionLoader::resolve_bin_directory(project_root, None)
                    .to_string_lossy()
                    .to_string()
            },
            |p| p.to_string_lossy().to_string(),
        ),
        storage: Some(ctx.storage_dir().to_string_lossy().to_string()),
        geoip_database: None,
        web_path: None,
    }
}

pub(super) fn security(api_external_url: &str) -> SecurityConfig {
    SecurityConfig {
        issuer: api_external_url.to_owned(),
        access_token_expiration: 2_592_000,
        refresh_token_expiration: 15_552_000,
        audiences: vec![
            JwtAudience::Web,
            JwtAudience::Api,
            JwtAudience::A2a,
            JwtAudience::Mcp,
        ],
        allowed_resource_audiences: default_resource_audiences(),
        allow_registration: true,
        login_page_url: None,
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        trusted_issuers: Vec::new(),
        id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
    }
}

pub(super) const fn runtime(environment: Environment, is_prod: bool) -> RuntimeConfig {
    RuntimeConfig {
        environment,
        log_level: if is_prod {
            LogLevel::Normal
        } else {
            LogLevel::Verbose
        },
        output_format: OutputFormat::Text,
        no_color: false,
        non_interactive: is_prod,
    }
}

pub(super) fn governance(api_internal_url: &str) -> GovernanceConfig {
    GovernanceConfig {
        authz: Some(AuthzConfig {
            hook: AuthzHookConfig {
                mode: AuthzMode::Webhook,
                url: Some(format!("{}/api/public/govern/authz", api_internal_url)),
                timeout_ms: 500,
                acknowledgement: None,
            },
        }),
    }
}
