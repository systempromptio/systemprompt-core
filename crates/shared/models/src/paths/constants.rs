//! Unified path constants for the SystemPrompt application.
//!
//! This module centralizes all path-related constants to ensure a single source
//! of truth across the codebase. Import from here rather than duplicating constants.

/// Directory names used throughout the application.
pub mod dir_names {
    /// The root directory name for SystemPrompt configuration.
    pub const SYSTEMPROMPT: &str = ".systemprompt";
    /// Directory containing profile configurations.
    pub const PROFILES: &str = "profiles";
    /// Directory containing Docker-related files.
    pub const DOCKER: &str = "docker";
    /// Directory for application storage.
    pub const STORAGE: &str = "storage";
}

/// File names used throughout the application.
pub mod file_names {
    /// Profile configuration file name.
    pub const PROFILE_CONFIG: &str = "profile.yaml";
    /// Profile secrets file name.
    pub const PROFILE_SECRETS: &str = "secrets.json";
    /// Cloud credentials file name.
    pub const CREDENTIALS: &str = "credentials.json";
    /// Tenants configuration file name.
    pub const TENANTS: &str = "tenants.json";
    /// CLI session state file name.
    pub const SESSION: &str = "session.json";
    /// Dockerfile name.
    pub const DOCKERFILE: &str = "Dockerfile";
    /// Docker entrypoint script name.
    pub const ENTRYPOINT: &str = "entrypoint.sh";
    /// Docker ignore file name.
    pub const DOCKERIGNORE: &str = "Dockerfile.dockerignore";
    /// Docker compose file name.
    pub const COMPOSE: &str = "compose.yaml";
}

/// Cloud container paths (used in Docker deployments).
pub mod cloud_container {
    /// Application root directory in container.
    pub const APP_ROOT: &str = "/app";
    /// Binary directory in container.
    pub const BIN: &str = "/app/bin";
    /// Services directory in container.
    pub const SERVICES: &str = "/app/services";
    /// Logs directory in container.
    pub const LOGS: &str = "/app/logs";
    /// Storage directory in container.
    pub const STORAGE: &str = "/app/storage";
    /// Web assets directory in container.
    pub const WEB: &str = "/app/web";
    /// Profiles directory in container.
    pub const PROFILES: &str = "/app/services/profiles";
}

/// Storage subdirectory structure.
pub mod storage {
    /// Base files directory.
    pub const FILES: &str = "files";
    /// Images subdirectory.
    pub const IMAGES: &str = "files/images";
    /// Generated images subdirectory.
    pub const GENERATED: &str = "files/images/generated";
    /// Logo images subdirectory.
    pub const LOGOS: &str = "files/images/logos";
    /// Audio files subdirectory.
    pub const AUDIO: &str = "files/audio";
    /// Video files subdirectory.
    pub const VIDEO: &str = "files/video";
    /// Documents subdirectory.
    pub const DOCUMENTS: &str = "files/documents";
    /// Uploads subdirectory.
    pub const UPLOADS: &str = "files/uploads";
    /// CSS files subdirectory.
    pub const CSS: &str = "files/css";
    /// JavaScript files subdirectory.
    pub const JS: &str = "files/js";
}

/// Build-related paths.
pub mod build {
    /// Cargo build target directory.
    pub const CARGO_TARGET: &str = "target";
    /// Web distribution directory.
    pub const WEB_DIST: &str = "core/web/dist";
    /// Web images source directory.
    pub const WEB_IMAGES: &str = "core/web/src/assets/images";
    /// Binary name.
    pub const BINARY_NAME: &str = "systemprompt";
}
