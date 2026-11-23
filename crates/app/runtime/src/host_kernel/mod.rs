//! [`HostKernel`]: the boot sequence of a least-privilege child process.
//!
//! A managed MCP server and a spawned agent boot from the environment the API
//! spawned them with: the profile path, the database URLs, the deployment's
//! public verification key set and the secrets their manifest grants. The
//! child admits the same composition the API admits ([`AdmitFn`], run once
//! against the loaded profile), seeds its kernel with [`base_kernel_state`]
//! (escalations go to the serving host's consumer,
//! `EscalationDelivery::Consumer`), resolves it, and then runs the stages
//! that read the resolved state — authz, governance, documents — over that
//! narrower input, so a child enforces the API's authorization by
//! construction. It never reads the profile's secrets source and builds no
//! OAuth repository.
//!
//! [`HostKernel::boot`] returns the composed handles in [`HostParts`],
//! including the resolved [`Kernel`] whose state the child's extension
//! context reads. The notifications member binds its own service slot while
//! the kernel resolves; the child binds only the files member's document
//! store slot.
//!
//! [`base_kernel_state`]: crate::base_kernel_state
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod boot;

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_config::{GrantedSecrets, GrantedSecretsError, LoadedProfile};
use systemprompt_database::DbPool;
use systemprompt_extension::Admitted;
use systemprompt_files::FilesConfig;
use systemprompt_governance::authz::{SharedAuthzHook, SubjectProviderSet};
use systemprompt_identifiers::UserId;
use systemprompt_loader::ServicesState;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile};
use systemprompt_mcp::ArtifactIngest;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_models::authz::SubjectSourceHandle;
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::{BackgroundTasks, BoxedSource, DynToolCallIntentClaims, FileStorage};
use systemprompt_users::UserRepository;

use crate::documents::DocumentsRuntime;
use crate::error::RuntimeError;
use crate::kernel::Kernel;

/// The admission a host runs once, against the loaded profile.
pub type AdmitFn = Box<dyn FnOnce(&Profile) -> Result<Admitted, BoxedSource> + Send>;

pub const PROFILE_ENV: &str = "SYSTEMPROMPT_PROFILE";

#[derive(Debug, thiserror::Error)]
pub enum HostBootError {
    #[error("{PROFILE_ENV} is not in the server's environment")]
    ProfileMissing,
    #[error("the profile could not be loaded")]
    Profile(#[source] Box<systemprompt_config::ConfigError>),
    #[error("the granted secrets could not be read")]
    Secrets(#[source] Box<GrantedSecretsError>),
    #[error("{VERIFICATION_KEYS_ENV} is not in the server's environment")]
    VerificationKeysMissing,
    #[error("{VERIFICATION_KEYS_ENV} is not a verification key set")]
    VerificationKeysInvalid(#[source] serde_json::Error),
    #[error("{VERIFICATION_KEYS_ENV} holds a key that cannot verify")]
    VerificationKey(#[source] jsonwebtoken::errors::Error),
    #[error("the services composition the API resolved could not be read")]
    ServicesRoot(#[source] Box<systemprompt_loader::BundleError>),
    #[error("the services configuration could not be loaded")]
    Services(#[source] Box<systemprompt_loader::ConfigLoadError>),
    #[error("the runtime configuration could not be built")]
    Config(#[source] Box<systemprompt_config::ConfigError>),
    #[error("the composition could not be admitted against the profile")]
    Admission(#[source] BoxedSource),
    #[error(transparent)]
    Runtime(Box<RuntimeError>),
}

impl From<RuntimeError> for HostBootError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(Box::new(error))
    }
}

/// The handles a child process composes from its granted environment.
#[derive(Clone)]
pub struct HostParts {
    pub profile: Arc<LoadedProfile>,
    pub services_root: ActiveServicesRoot,
    pub config: Arc<Config>,
    pub app_paths: Arc<AppPaths>,
    pub files_config: Arc<FilesConfig>,
    pub database: DbPool,
    pub services: ServicesState,
    pub secrets: Arc<GrantedSecrets>,
    pub verification: VerificationKeyring,
    pub authz_hook: SharedAuthzHook,
    pub subject_providers: SubjectProviderSet,
    pub subjects: SubjectSourceHandle,
    pub artifact_ingest: Arc<ArtifactIngest>,
    pub tool_call_intents: DynToolCallIntentClaims,
    pub mcp_sessions: Arc<McpSessionRepository>,
    pub user_repository: Arc<UserRepository>,
    pub documents: Arc<DocumentsRuntime>,
    pub background_tasks: BackgroundTasks,
    pub log_sink: systemprompt_logging::LogSink,
    pub tokens: Arc<VerifyingTokenAuthority>,
    pub file_storage: Arc<dyn FileStorage>,
    pub kernel: Arc<Kernel>,
    pub admitted: Arc<Admitted>,
    pub system_owner: UserId,
}

impl std::fmt::Debug for HostParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostParts")
            .field("profile", &self.profile.path())
            .field("secrets", &self.secrets)
            .finish_non_exhaustive()
    }
}

impl HostParts {
    #[must_use]
    pub fn kernel_state(&self) -> &Dependencies {
        self.kernel.state()
    }
}

/// The boot sequence of a child host; see the module documentation.
#[derive(Debug, Clone, Copy)]
pub struct HostKernel;
