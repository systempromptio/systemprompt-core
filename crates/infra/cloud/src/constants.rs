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
    pub const PROFILE_DOCKER_DIR: &str = "docker";
    pub const ENTRYPOINT: &str = "entrypoint.sh";
    pub const DOCKERIGNORE: &str = "Dockerfile.dockerignore";
    pub const COMPOSE_FILE: &str = "compose.yaml";
}

pub mod build {
    pub const CARGO_TARGET: &str = "target";
    pub const WEB_DIST: &str = "core/web/dist";
    pub const WEB_IMAGES: &str = "core/web/src/assets/images";
    pub const BINARY_NAME: &str = "systemprompt";
    pub const DOCKERFILE: &str = ".systemprompt/Dockerfile";
}

pub mod profile {
    pub const DEFAULT_DB_TYPE: &str = "postgres";
    pub const DEFAULT_PORT: u16 = 8080;
    pub const LOCAL_HOST: &str = "127.0.0.1";
    pub const CLOUD_HOST: &str = "0.0.0.0";
    pub const DEFAULT_CLOUD_URL: &str = "https://cloud.systemprompt.io";
    pub const LOCAL_ISSUER: &str = "systemprompt-local";
    pub const CLOUD_ISSUER: &str = "systemprompt";
    pub const ACCESS_TOKEN_EXPIRATION: i64 = 86400;
    pub const REFRESH_TOKEN_EXPIRATION: i64 = 2_592_000;
    pub const CLOUD_APP_PATH: &str = "/app";
    pub const CREDENTIALS_PATH: &str = "../../credentials.json";
    pub const TENANTS_PATH: &str = "../../tenants.json";
}
