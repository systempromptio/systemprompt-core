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
use systemprompt_manifest::profile::{
    AuditConfig, AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, QuotaConfig,
    default_resource_audiences,
};
use systemprompt_manifest::{
    Environment, LogLevel, OutputFormat, PathsConfig, RuntimeConfig, SecurityConfig, ServerConfig,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::defaults::{DEFAULT_API_PORT, DEFAULT_API_URL};

pub(super) fn server(is_prod: bool) -> ServerConfig {
    let host = if is_prod { "0.0.0.0" } else { "127.0.0.1" };
    let mut server = ServerConfig::new(host, DEFAULT_API_PORT, DEFAULT_API_URL);
    server.extra_cors_origins = vec![
        "http://localhost:5173".to_owned(),
        format!("http://127.0.0.1:{DEFAULT_API_PORT}"),
    ];
    server.role = systemprompt_manifest::profile::NodeRole::All;
    server
}

pub(super) fn paths(
    project_root: &Path,
    bin_path: Option<&Path>,
    ctx: &ProjectContext,
) -> PathsConfig {
    PathsConfig {
        root: None,
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
        data: None,
        web_path: None,
        shipped_files: None,
    }
}

pub(super) fn security() -> SecurityConfig {
    SecurityConfig {
        issuer: String::new(),
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
        allow_dynamic_client_registration: true,
        login_page_url: None,
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        trusted_issuers: Vec::new(),
        id_jag_ttl_secs: systemprompt_manifest::profile::DEFAULT_ID_JAG_TTL_SECS,
        api_keys: systemprompt_manifest::profile::ApiKeyPolicyConfig::default(),
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

pub(super) fn governance() -> GovernanceConfig {
    GovernanceConfig {
        authz: Some(AuthzConfig {
            hook: AuthzHookConfig {
                mode: AuthzMode::Webhook,
                url: None,
                timeout_ms: 500,
                acknowledgement: None,
            },
        }),
        audit: AuditConfig::default(),
        quota: QuotaConfig::default(),
        unapplied_planes: systemprompt_manifest::profile::UnappliedPlaneMode::default(),
    }
}
