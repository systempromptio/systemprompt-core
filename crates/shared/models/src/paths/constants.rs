pub mod dir_names {
    pub const SYSTEMPROMPT: &str = ".systemprompt";
    pub const PROFILES: &str = "profiles";
    pub const DOCKER: &str = "docker";
    pub const STORAGE: &str = "storage";
    pub const SESSIONS: &str = "sessions";
}

pub mod file_names {
    pub const PROFILE_CONFIG: &str = "profile.yaml";
    pub const PROFILE_SECRETS: &str = "secrets.json";
    pub const CREDENTIALS: &str = "credentials.json";
    pub const TENANTS: &str = "tenants.json";
    pub const SESSION: &str = "session.json";
    pub const SESSIONS_INDEX: &str = "index.json";
    pub const DOCKERFILE: &str = "Dockerfile";
    pub const ENTRYPOINT: &str = "entrypoint.sh";
    pub const DOCKERIGNORE: &str = "Dockerfile.dockerignore";
    pub const COMPOSE: &str = "compose.yaml";
}

pub mod cloud_container {
    pub const APP_ROOT: &str = "/app";
    pub const BIN: &str = "/app/bin";
    pub const SERVICES: &str = "/app/services";
    pub const LOGS: &str = "/app/logs";
    pub const STORAGE: &str = "/app/storage";
    pub const WEB: &str = "/app/web";
    pub const PROFILES: &str = "/app/services/profiles";
    pub const TEMPLATES: &str = "/app/services/web/templates";
    pub const ASSETS: &str = "/app/services/web/assets";
}

pub mod storage {
    pub const FILES: &str = "files";
    pub const IMAGES: &str = "files/images";
    pub const GENERATED: &str = "files/images/generated";
    pub const LOGOS: &str = "files/images/logos";
    pub const AUDIO: &str = "files/audio";
    pub const VIDEO: &str = "files/video";
    pub const DOCUMENTS: &str = "files/documents";
    pub const UPLOADS: &str = "files/uploads";
    pub const CSS: &str = "files/css";
    pub const JS: &str = "files/js";
}

pub mod build {
    pub const CARGO_TARGET: &str = "target";
    pub const BINARY_NAME: &str = "systemprompt";
}
