//! Compile-time constants for the cloud layer: container paths, callback ports
//! and timeouts, API endpoints, profile defaults and trusted-proxy ranges.
//!
//! Grouped into submodules by concern (`oauth`, `credentials`, `api`,
//! `profile`, `proxies`, `env_vars`); the on-disk file names are
//! `systemprompt_loader::workspace::constants`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_manifest::paths::constants::{build, dir_names, file_names, storage};

pub mod container {
    use systemprompt_manifest::paths::constants::cloud_container;

    pub const APP: &str = cloud_container::APP_ROOT;
    pub const APP_ROOT: &str = cloud_container::APP_ROOT;
    pub const BIN: &str = cloud_container::BIN;
    pub const LOGS: &str = cloud_container::LOGS;
    pub const SERVICES: &str = cloud_container::SERVICES;
    pub const SERVICES_CACHE: &str = cloud_container::SERVICES_CACHE;
    pub const STORAGE: &str = cloud_container::STORAGE;
    pub const WEB: &str = cloud_container::WEB;
    pub const WEB_DIST: &str = cloud_container::WEB_DIST;
    pub const WEB_CONFIG: &str = cloud_container::WEB_CONFIG;
    pub const PROFILES: &str = cloud_container::PROFILES;
    pub const TEMPLATES: &str = cloud_container::TEMPLATES;
    pub const ASSETS: &str = cloud_container::ASSETS;
}

pub const CALLBACK_TIMEOUT_SECS: u64 = 300;

pub mod oauth {
    pub const CALLBACK_PORT: u16 = 8765;
    pub const CALLBACK_TIMEOUT_SECS: u64 = super::CALLBACK_TIMEOUT_SECS;
}

pub mod credentials {
    pub const VALIDATION_TTL_SECS: i64 = 900;
}

pub mod api {
    pub const PRODUCTION_URL: &str = "https://api.systemprompt.io";
    pub const SANDBOX_URL: &str = "https://api-sandbox.systemprompt.io";
}

pub mod profile {
    use super::container;

    pub const DEFAULT_DB_TYPE: &str = "postgres";
    pub const DEFAULT_PORT: u16 = systemprompt_models::defaults::DEFAULT_API_PORT;
    pub const LOCAL_HOST: &str = "127.0.0.1";
    pub const CLOUD_HOST: &str = "0.0.0.0";
    pub const DEFAULT_CLOUD_URL: &str = "https://cloud.systemprompt.io";
    pub const ACCESS_TOKEN_EXPIRATION: i64 = 2_592_000;
    pub const REFRESH_TOKEN_EXPIRATION: i64 = 15_552_000;
    pub const CLOUD_APP_PATH: &str = container::APP_ROOT;
    pub const CREDENTIALS_PATH: &str = "../../credentials.json";
    pub const TENANTS_PATH: &str = "../../tenants.json";
}

pub mod proxies {
    pub const PRIVATE_RANGES: &[&str] = &[
        "127.0.0.0/8",
        "::1/128",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
    ];

    pub const FLY_PRIVATE_RANGES: &[&str] = &["fc00::/7"];

    pub const FLY_PUBLIC_RANGES: &[&str] = &["66.241.64.0/18"];

    pub const GOOGLE_LB_RANGES: &[&str] = &["35.191.0.0/16", "130.211.0.0/22"];

    pub const CLOUDFLARE_RANGES: &[&str] = &[
        "173.245.48.0/20",
        "103.21.244.0/22",
        "103.22.200.0/22",
        "103.31.4.0/22",
        "141.101.64.0/18",
        "108.162.192.0/18",
        "190.93.240.0/20",
        "188.114.96.0/20",
        "197.234.240.0/22",
        "198.41.128.0/17",
        "162.158.0.0/15",
        "104.16.0.0/13",
        "104.24.0.0/14",
        "172.64.0.0/13",
        "131.0.72.0/22",
        "2400:cb00::/32",
        "2606:4700::/32",
        "2803:f800::/32",
        "2405:b500::/32",
        "2405:8100::/32",
        "2a06:98c0::/29",
        "2c0f:f248::/32",
    ];
}

pub mod env_vars {
    pub use systemprompt_manifest::paths::constants::env_vars::CUSTOM_SECRETS;

    pub const SYSTEM_MANAGED: &[&str] = &[
        systemprompt_models::subprocess::DEPLOYMENT_HOST_ENV,
        "FLY_APP_NAME",
        "FLY_MACHINE_ID",
        "POD_NAME",
    ];

    pub const CLI_SYNCED: &[&str] = &[
        "SYSTEMPROMPT_API_TOKEN",
        "SYSTEMPROMPT_USER_EMAIL",
        "SYSTEMPROMPT_CLI_REMOTE",
        "SYSTEMPROMPT_PROFILE",
    ];

    pub fn is_system_managed(key: &str) -> bool {
        SYSTEM_MANAGED.iter().any(|&k| k.eq_ignore_ascii_case(key))
    }
}
