//! Typed error boundary for the runtime crate.
//!
//! All public APIs of `systemprompt-runtime` return [`RuntimeResult<T>`]
//! (i.e. `Result<T, RuntimeError>`). [`RuntimeError`] composes the typed
//! errors of upstream layers (config, database, events, files, users,
//! extensions) via `#[from]` so callers can pattern-match on the original
//! cause without losing fidelity.
//!
//! Boot steps whose failure needs more context than the upstream error
//! carries (a storage root, a bundle name) get a struct variant holding that
//! context next to the `#[source]` cause.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_agent::AgentError;
use systemprompt_analytics::AnalyticsError;
use systemprompt_config::paths::PathError;
use systemprompt_config::{
    ConfigError as ProfileConfigError, ProfileBootstrapError, SecretsBootstrapError,
};
use systemprompt_content::ContentError;
use systemprompt_extension::LoaderError;
use systemprompt_files::FilesError;
use systemprompt_identifiers::SecretName;
use systemprompt_loader::{BundleError, ConfigLoadError};
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_mcp::McpDomainError;
use systemprompt_models::errors::GlobalConfigError;
use systemprompt_oauth::OauthError;
use systemprompt_security::authz::AuthzError;
use systemprompt_security::keys::TokenAuthorityError;
use systemprompt_security::policy::GovernanceEngineError;
use systemprompt_traits::{BoxedSource, FileStorageError, RepositoryError};
use systemprompt_users::UserError;
use thiserror::Error;

pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Profile(#[from] ProfileConfigError),

    #[error(transparent)]
    ProfileBootstrap(#[from] ProfileBootstrapError),

    #[error(transparent)]
    Config(#[from] GlobalConfigError),

    #[error(transparent)]
    Paths(#[from] PathError),

    #[error(transparent)]
    Files(#[from] FilesError),

    #[error(transparent)]
    Users(#[from] UserError),

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[error(transparent)]
    Analytics(#[from] AnalyticsError),

    #[error(transparent)]
    Mcp(#[from] McpDomainError),

    #[error(transparent)]
    Agent(#[from] AgentError),

    #[error(transparent)]
    Content(#[from] ContentError),

    #[error(transparent)]
    Oauth(#[from] OauthError),

    #[error(transparent)]
    Loader(#[from] LoaderError),

    #[error(transparent)]
    Governance(#[from] GovernanceEngineError),

    #[error(transparent)]
    Managed(#[from] ManagedError),

    #[error(transparent)]
    Secrets(#[from] SecretsBootstrapError),

    #[error("services config: {0}")]
    ServicesConfig(#[from] ConfigLoadError),

    #[error("services bundle: {0}")]
    ServicesBundle(#[from] BundleError),

    #[error("services bundle {name} has no cached fetch state")]
    ServicesBundleNotCached { name: String },

    #[error("services bundle {name} manifest: {source}")]
    ServicesBundleManifest {
        name: String,
        #[source]
        source: BundleError,
    },

    #[error("services authz reconcile: {0}")]
    ServicesReconcile(#[source] AuthzError),

    #[error("services reconcile state: {0}")]
    ServicesReconcileState(#[source] BundleError),

    #[error("signing key init: {0}")]
    Signing(#[from] TokenAuthorityError),

    #[error("authz bootstrap: {0}")]
    Authz(#[from] AuthzError),

    #[error("storage root {} probe: {source}", .path.display())]
    StorageProbe {
        path: PathBuf,
        #[source]
        source: FileStorageError,
    },

    #[error("storage root {} did not read back what was written", .path.display())]
    StorageReadBack { path: PathBuf },

    #[error("storage.credentials names secret '{name}', which the secrets store does not hold")]
    StorageCredentialMissing { name: SecretName },

    #[error("storage.credentials secret '{name}' is not a service-account key: {source}")]
    StorageCredential {
        name: SecretName,
        #[source]
        source: serde_json::Error,
    },

    #[error("storage.backend 'gcs' requires storage.bucket")]
    StorageBucketMissing,

    #[error("storage endpoint: {0}")]
    StorageEndpoint(#[source] url::ParseError),

    #[error("storage HTTP client: {0}")]
    StorageHttp(#[source] reqwest::Error),

    #[error(
        "Configured system admin '{username}' was not found in the users table. Run `systemprompt \
         admin bootstrap` first."
    )]
    SystemAdminNotFound { username: String },

    #[error(
        "Configured system admin '{username}' exists but is not active. Re-activate the user \
         before starting the platform."
    )]
    SystemAdminInactive { username: String },

    #[error(
        "Configured system admin '{username}' exists but does not carry the 'admin' role. Grant \
         the role before starting the platform."
    )]
    SystemAdminMissingRole { username: String },

    #[error(
        "Configured GeoIP database at '{path}' could not be loaded: {source}. Fix or remove \
         paths.geoip_database from the profile."
    )]
    GeoIpUnreadable {
        path: String,
        #[source]
        source: BoxedSource,
    },

    #[error("DATABASE_URL is empty")]
    EmptyDatabaseUrl,

    #[error("DATABASE_URL must be a postgres:// or postgresql:// URL")]
    UnsupportedDatabaseUrl,
}
