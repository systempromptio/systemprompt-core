//! Error types for the systemprompt-sync crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum SyncError {
        common: [io, http, json, yaml],

        #[error("API error {status}: {message}")]
        ApiError { status: u16, message: String },

        #[error("Unauthorized - run 'systemprompt cloud login'")]
        Unauthorized,

        #[error("Tenant has no associated app")]
        TenantNoApp,

        #[error("Must run from project root (with infrastructure/ directory)")]
        NotProjectRoot,

        #[error("Command failed: {command}")]
        CommandFailed { command: String },

        #[error("Failed to spawn `{command}`: {source}")]
        CommandSpawnFailed {
            command: String,
            #[source]
            source: std::io::Error,
        },

        #[error("Failed to open file {path}: {source}")]
        FileOpenFailed {
            path: String,
            #[source]
            source: std::io::Error,
        },

        #[error("Docker login failed")]
        DockerLoginFailed,

        #[error("Git SHA unavailable")]
        GitShaUnavailable,

        #[error("Missing configuration: {0}")]
        MissingConfig(String),

        #[error("Partial import failure after {completed}/{total} items: {message}")]
        PartialImport {
            completed: usize,
            total: usize,
            message: String,
        },

        #[error("Unsafe tarball entry rejected: {0}")]
        TarballUnsafe(String),

        #[error("Database error: {0}")]
        Database(#[from] sqlx::Error),

        #[error("Path error: {0}")]
        StripPrefix(#[from] std::path::StripPrefixError),

        #[error("Zip error: {0}")]
        Zip(#[from] zip::result::ZipError),

        #[error("Invalid input: {0}")]
        InvalidInput(String),

        #[error("internal: {0}")]
        Internal(String),

        #[error("{0}")]
        Cloud(#[from] systemprompt_cloud::CloudError),

        #[error("{0}")]
        ConfigLoad(#[from] systemprompt_loader::ConfigLoadError),

        #[error("{0}")]
        ExtensionDiscovery(#[from] systemprompt_extension::LoaderError),

        #[error("Hostname not configured for tenant.\nRun: systemprompt cloud login")]
        HostnameNotConfigured,

        #[error("Pre-deploy sync failed. Use --no-sync to skip (WARNING: may lose data).")]
        PreDeploySyncFailed,

        #[error("{stage} failed: {source}")]
        PreSyncStage {
            stage: &'static str,
            #[source]
            source: Box<SyncError>,
        },

        #[error("{0}")]
        BuildArtifacts(String),
    }
}

impl SyncError {
    pub fn internal(cause: impl std::fmt::Display) -> Self {
        Self::Internal(cause.to_string())
    }

    pub fn pre_sync_stage(stage: &'static str, source: Self) -> Self {
        Self::PreSyncStage {
            stage,
            source: Box::new(source),
        }
    }

    pub fn invalid_input(cause: impl std::fmt::Display) -> Self {
        Self::InvalidInput(cause.to_string())
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Http(_))
            || matches!(
                self,
                Self::ApiError { status, .. }
                    if *status == 502 || *status == 503 || *status == 504 || *status == 429
            )
    }
}

pub type SyncResult<T> = Result<T, SyncError>;
