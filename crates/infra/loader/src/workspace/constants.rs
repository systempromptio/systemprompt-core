//! On-disk names of the workspace layer: the credentials, tenants, operator
//! and session files, the local database container, and the profile defaults.
//!
//! Path-name and storage constants are re-exported from
//! `systemprompt_manifest`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_manifest::paths::constants::{build, dir_names, file_names, storage};

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

pub mod operator {
    use super::dir_names;

    pub const DEFAULT_DIR_NAME: &str = dir_names::SYSTEMPROMPT;
    pub const DEFAULT_FILE_NAME: &str = "operator_credentials.json";
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
