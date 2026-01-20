//! Constants for cloud infrastructure.
//!
//! Re-exports unified path constants from `systemprompt_models` and provides
//! cloud-specific constants for OAuth, API endpoints, and Docker configuration.

// Re-export unified path constants from shared models
pub use systemprompt_models::paths::constants::{build, dir_names, file_names, storage};

/// Container path constants with backward-compatible aliases.
pub mod container {
    use systemprompt_models::paths::constants::cloud_container;

    pub const APP: &str = cloud_container::APP_ROOT;
    pub const APP_ROOT: &str = cloud_container::APP_ROOT;
    pub const BIN: &str = cloud_container::BIN;
    pub const LOGS: &str = cloud_container::LOGS;
    pub const SERVICES: &str = cloud_container::SERVICES;
    pub const STORAGE: &str = cloud_container::STORAGE;
    pub const WEB: &str = cloud_container::WEB;
    pub const PROFILES: &str = cloud_container::PROFILES;
    pub const TEMPLATES: &str = cloud_container::TEMPLATES;
    pub const ASSETS: &str = cloud_container::ASSETS;
}

pub mod oauth {
    pub const CALLBACK_PORT: u16 = 8765;
    pub const CALLBACK_TIMEOUT_SECS: u64 = 300;
}

pub mod checkout {
    pub const CALLBACK_PORT: u16 = 8766;
    pub const CALLBACK_TIMEOUT_SECS: u64 = 300;
    pub const PROVISIONING_POLL_INTERVAL_MS: u64 = 2000;
}

pub mod credentials {
    use super::{dir_names, file_names};

    pub const DEFAULT_DIR_NAME: &str = dir_names::SYSTEMPROMPT;
    pub const DEFAULT_FILE_NAME: &str = file_names::CREDENTIALS;
}

pub mod tenants {
    use super::{dir_names, file_names};

    pub const DEFAULT_DIR_NAME: &str = dir_names::SYSTEMPROMPT;
    pub const DEFAULT_FILE_NAME: &str = file_names::TENANTS;
}

pub mod cli_session {
    use super::{dir_names, file_names};

    pub const DEFAULT_DIR_NAME: &str = dir_names::SYSTEMPROMPT;
    pub const DEFAULT_FILE_NAME: &str = file_names::SESSION;
}

pub mod docker {
    pub const CONTAINER_NAME_PREFIX: &str = "systemprompt-postgres";
    pub const COMPOSE_PATH: &str = "infrastructure/docker";

    pub fn container_name(env_name: &str) -> String {
        format!("{}-{}", CONTAINER_NAME_PREFIX, env_name)
    }
}

pub mod api {
    pub const PRODUCTION_URL: &str = "https://api.systemprompt.io";
    pub const SANDBOX_URL: &str = "https://api-sandbox.systemprompt.io";
}

pub mod regions {
    pub const AVAILABLE: &[(&str, &str)] = &[
        ("iad", "US East (Virginia)"),
        ("lhr", "Europe (London)"),
        ("fra", "Europe (Frankfurt)"),
        ("ams", "Europe (Amsterdam)"),
        ("sin", "Asia (Singapore)"),
        ("nrt", "Asia (Tokyo)"),
        ("syd", "Australia (Sydney)"),
        ("gru", "South America (São Paulo)"),
    ];
}

/// Path constants - re-exports from unified module with backward-compatible
/// names.
pub mod paths {
    use super::{dir_names, file_names};

    pub const ROOT_DIR: &str = dir_names::SYSTEMPROMPT;
    pub const PROFILES_DIR: &str = dir_names::PROFILES;
    pub const DOCKER_DIR: &str = dir_names::DOCKER;
    pub const STORAGE_DIR: &str = dir_names::STORAGE;
    pub const DOCKERFILE: &str = file_names::DOCKERFILE;
    pub const PROFILE_CONFIG: &str = file_names::PROFILE_CONFIG;
    pub const PROFILE_SECRETS: &str = file_names::PROFILE_SECRETS;
    pub const CREDENTIALS_FILE: &str = file_names::CREDENTIALS;
    pub const TENANTS_FILE: &str = file_names::TENANTS;
    pub const SESSION_FILE: &str = file_names::SESSION;
    pub const PROFILE_DOCKER_DIR: &str = dir_names::DOCKER;
    pub const ENTRYPOINT: &str = file_names::ENTRYPOINT;
    pub const DOCKERIGNORE: &str = file_names::DOCKERIGNORE;
    pub const COMPOSE_FILE: &str = file_names::COMPOSE;
}

pub mod profile {
    use super::container;

    pub const DEFAULT_DB_TYPE: &str = "postgres";
    pub const DEFAULT_PORT: u16 = 8080;
    pub const LOCAL_HOST: &str = "127.0.0.1";
    pub const CLOUD_HOST: &str = "0.0.0.0";
    pub const DEFAULT_CLOUD_URL: &str = "https://cloud.systemprompt.io";
    pub const LOCAL_ISSUER: &str = "systemprompt-local";
    pub const CLOUD_ISSUER: &str = "systemprompt";
    /// Access token expiration in seconds. Default: 30 days.
    pub const ACCESS_TOKEN_EXPIRATION: i64 = 2_592_000;
    /// Refresh token expiration in seconds. Default: 180 days.
    pub const REFRESH_TOKEN_EXPIRATION: i64 = 15_552_000;
    pub const CLOUD_APP_PATH: &str = container::APP_ROOT;
    pub const CREDENTIALS_PATH: &str = "../../credentials.json";
    pub const TENANTS_PATH: &str = "../../tenants.json";
}

pub mod env_vars {
    /// System-managed environment variables that should never be set by users.
    /// These are managed by the cloud infrastructure or deployment process.
    pub const SYSTEM_MANAGED: &[&str] = &[
        "DATABASE_URL",
        "JWT_SECRET",
        "SYSTEMPROMPT_API_URL",
        "FLY_APP_NAME",
        "FLY_MACHINE_ID",
        "APP_URL",
        "API_EXTERNAL_URL",
    ];

    /// Environment variables synced by the CLI during deployment.
    /// These are set automatically and should not be manually modified.
    pub const CLI_SYNCED: &[&str] = &[
        "SYSTEMPROMPT_API_TOKEN",
        "SYSTEMPROMPT_USER_EMAIL",
        "SYSTEMPROMPT_CLI_REMOTE",
        "SYSTEMPROMPT_PROFILE",
    ];

    /// Check if a key is a system-managed variable (case-insensitive).
    pub fn is_system_managed(key: &str) -> bool {
        SYSTEM_MANAGED.iter().any(|&k| k.eq_ignore_ascii_case(key))
    }
}
