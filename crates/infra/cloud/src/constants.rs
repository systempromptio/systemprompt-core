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
    pub const DEFAULT_DIR_NAME: &str = ".systemprompt";
    pub const DEFAULT_FILE_NAME: &str = "credentials.json";
}

pub mod tenants {
    pub const DEFAULT_DIR_NAME: &str = ".systemprompt";
    pub const DEFAULT_FILE_NAME: &str = "tenants.json";
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

pub mod paths {
    pub const ROOT_DIR: &str = ".systemprompt";
    pub const PROFILES_DIR: &str = "profiles";
    pub const DOCKER_DIR: &str = "docker";
    pub const STORAGE_DIR: &str = "storage";
    pub const DOCKERFILE: &str = "Dockerfile";
    pub const PROFILE_CONFIG: &str = "profile.yaml";
    pub const PROFILE_SECRETS: &str = "secrets.json";
    pub const CREDENTIALS_FILE: &str = "credentials.json";
    pub const TENANTS_FILE: &str = "tenants.json";
}

pub mod build {
    pub const CARGO_TARGET: &str = "target";
    pub const WEB_DIST: &str = "core/web/dist";
    pub const WEB_IMAGES: &str = "core/web/src/assets/images";
    pub const BINARY_NAME: &str = "systemprompt";
    pub const DOCKERFILE: &str = ".systemprompt/Dockerfile";
}
