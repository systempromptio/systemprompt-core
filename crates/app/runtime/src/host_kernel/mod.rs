//! [`HostKernel`]: the boot sequence of a least-privilege child process.
//!
//! A managed MCP server and a spawned agent boot from the environment the API
//! spawned them with: the profile path, the database URLs, the deployment's
//! public verification key set and the secrets their manifest grants. The
//! sequence runs the same composition stages as [`AppContextBuilder`]
//! (database, authz, governed extensions, event plane, documents,
//! notifications) over that narrower input, so a child enforces the API's
//! authorization by construction. It never reads the profile's secrets
//! source and builds no OAuth repository.
//!
//! [`HostKernel::boot`] returns the composed handles in [`HostParts`] and a
//! [`HostKernel`] that finishes the two steps that need the composed context
//! itself as an [`ExtensionContext`]: binding the extension state and
//! assembling notifications over the discovered extension seams.
//!
//! [`AppContextBuilder`]: crate::AppContextBuilder
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod boot;

use std::sync::Arc;

use systemprompt_agent::repository::A2ARepositories;
use systemprompt_config::paths::AppPaths;
use systemprompt_config::{GrantedSecrets, GrantedSecretsError, LoadedProfile};
use systemprompt_database::DbPool;
use systemprompt_extension::{ExtensionContext, ExtensionRegistry, StateHandle};
use systemprompt_files::FilesConfig;
use systemprompt_governance::authz::{SharedAuthzHook, SubjectProviderSet};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_loader::ServicesState;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_marketplace::managed::PublicationPolicy;
use systemprompt_mcp::ArtifactIngest;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_models::authz::SubjectSourceHandle;
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
#[cfg(feature = "notifications")]
use systemprompt_notifications::service::NotificationsService;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::{BackgroundTasks, DynToolCallIntentClaims, FileStorage};
use systemprompt_users::UserRepository;

#[cfg(feature = "notifications")]
use crate::builder::notifications::{ExtensionSeams, NotificationsCore};
use crate::documents::DocumentsRuntime;
#[cfg(feature = "notifications")]
use crate::documents::ExtensionAudiences;
use crate::error::RuntimeError;

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
    pub publication: PublicationPolicy,
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
    pub extensions: Arc<ExtensionRegistry>,
    pub system_owner: UserId,
    pub a2a: Arc<A2ARepositories>,
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
    pub fn kernel_state(&self) -> Dependencies {
        crate::base_kernel_state(
            &self.database,
            &self.app_paths,
            &self.files_config,
            self.profile.profile(),
            &self.documents,
        )
    }
}

/// Finishes a child's composition once the context exists.
pub struct HostKernel {
    market_state: StateHandle,
    extensions: Arc<ExtensionRegistry>,
    disabled_extensions: Vec<ExtensionId>,
    #[cfg(feature = "notifications")]
    audiences: ExtensionAudiences,
    #[cfg(feature = "notifications")]
    notifications: NotificationsCore,
}

impl std::fmt::Debug for HostKernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostKernel")
            .field("disabled_extensions", &self.disabled_extensions)
            .finish_non_exhaustive()
    }
}

impl HostKernel {
    pub fn bind_extension_state(
        &self,
        ctx: &dyn ExtensionContext,
        kernel: Dependencies,
    ) -> Result<Dependencies, HostBootError> {
        let state = self
            .extensions
            .build_state(ctx, kernel, &self.disabled_extensions)
            .map_err(RuntimeError::from)?;
        self.market_state
            .bind(state.clone())
            .map_err(RuntimeError::from)?;
        Ok(state)
    }

    #[must_use]
    pub fn notifications_without_seams(&self) -> HostNotifications {
        #[cfg(feature = "notifications")]
        {
            HostNotifications(Arc::clone(
                &self
                    .notifications
                    .assemble(ExtensionSeams::default())
                    .service,
            ))
        }
        #[cfg(not(feature = "notifications"))]
        {
            HostNotifications(())
        }
    }

    #[cfg_attr(
        not(feature = "notifications"),
        expect(
            unused_variables,
            reason = "extension seams are discovered only by the notifications assembly"
        )
    )]
    #[must_use]
    pub fn notifications(&self, ctx: &dyn ExtensionContext) -> HostNotifications {
        #[cfg(feature = "notifications")]
        {
            let seams = ExtensionSeams::discover(ctx);
            self.audiences.bind(seams.audiences.clone());
            HostNotifications(Arc::clone(&self.notifications.assemble(seams).service))
        }
        #[cfg(not(feature = "notifications"))]
        {
            HostNotifications(())
        }
    }
}

/// A child host's notifications service, present when the runtime is built
/// with notifications; the host leaves escalated delivery to the API's
/// consumer.
#[derive(Clone)]
#[cfg_attr(not(feature = "notifications"), derive(Copy))]
pub struct HostNotifications(
    #[cfg(feature = "notifications")] Arc<NotificationsService>,
    #[cfg(not(feature = "notifications"))] (),
);

impl std::fmt::Debug for HostNotifications {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostNotifications").finish_non_exhaustive()
    }
}

impl HostNotifications {
    #[cfg(feature = "notifications")]
    #[must_use]
    pub const fn service(&self) -> &Arc<NotificationsService> {
        &self.0
    }
}
