//! Booting a [`HostContext`] from a spawned server's environment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_config::{GrantedSecrets, GrantedSecretsError, LoadedProfile};
use systemprompt_database::MigrationConfig;
use systemprompt_loader::{ConfigLoader, ServicesSourceBootstrap, ServicesState};
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_security::keys::{TokenVerificationKeys, VerificationKeyring};
use systemprompt_traits::{BackgroundTasks, CacheEpochs, SecretStore};
use systemprompt_users::UserRepository;

use super::HostContext;
use crate::builder::authz::authz_stage;
use crate::builder::composition::build_artifact_ingest;
use crate::builder::core_layer::{SchemaPolicy, init_files_config};
use crate::builder::database::connect_database;
use crate::builder::documents::{DocumentsInputs, build_documents};
use crate::builder::events::event_plane;
use crate::builder::governance::init_governed_extensions;
use crate::documents::ExtensionAudiences;
use crate::error::RuntimeError;

pub const PROFILE_ENV: &str = "SYSTEMPROMPT_PROFILE";

#[derive(Debug, thiserror::Error)]
pub enum HostBootError {
    #[error("{PROFILE_ENV} is not in the server's environment")]
    ProfileMissing,
    #[error("the profile could not be loaded")]
    Profile(#[source] systemprompt_config::ConfigError),
    #[error("the granted secrets could not be read")]
    Secrets(#[source] GrantedSecretsError),
    #[error("{VERIFICATION_KEYS_ENV} is not in the server's environment")]
    VerificationKeysMissing,
    #[error("{VERIFICATION_KEYS_ENV} is not a verification key set")]
    VerificationKeysInvalid(#[source] serde_json::Error),
    #[error("{VERIFICATION_KEYS_ENV} holds a key that cannot verify")]
    VerificationKey(#[source] jsonwebtoken::errors::Error),
    #[error("the services root could not be resolved")]
    ServicesRoot(#[source] systemprompt_loader::BundleError),
    #[error("the services configuration could not be loaded")]
    Services(#[source] systemprompt_loader::ConfigLoadError),
    #[error("the runtime configuration could not be built")]
    Config(#[source] systemprompt_config::ConfigError),
    #[error(transparent)]
    Runtime(Box<RuntimeError>),
}

impl From<RuntimeError> for HostBootError {
    fn from(error: RuntimeError) -> Self {
        Self::Runtime(Box::new(error))
    }
}

impl HostContext {
    pub async fn from_process_env() -> Result<Self, HostBootError> {
        Self::boot(|name| std::env::var(name).ok()).await
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the one assembly sequence of the host context; splitting it threads a dozen handles through helper signatures"
    )]
    pub async fn boot(
        lookup: impl Fn(&str) -> Option<String> + Send + Sync,
    ) -> Result<Self, HostBootError> {
        let profile_path = lookup(PROFILE_ENV).ok_or(HostBootError::ProfileMissing)?;
        let loaded =
            LoadedProfile::from_path(Path::new(&profile_path)).map_err(HostBootError::Profile)?;
        let profile = loaded.profile();
        let secrets =
            Arc::new(GrantedSecrets::from_lookup(&lookup).map_err(HostBootError::Secrets)?);
        let verification = verification_keyring(&lookup)?;

        let services_root = ServicesSourceBootstrap::resolve(
            profile,
            |name| secrets.get(name).map(str::to_owned),
            env!("CARGO_PKG_VERSION"),
        )
        .await
        .map_err(HostBootError::ServicesRoot)?;
        let app_paths = Arc::new(
            AppPaths::from_profile(
                &profile.paths,
                profile.path_resolution(),
                Some(services_root.path.as_path()),
            )
            .map_err(RuntimeError::from)?,
        );
        let config = Arc::new(
            systemprompt_config::build_from_profile(
                &loaded,
                secrets.credentials(),
                Some(services_root.path.as_path()),
            )
            .map_err(HostBootError::Config)?,
        );
        let files_config = init_files_config(profile, &app_paths)?;
        let services_config = ConfigLoader::for_profile(profile, &services_root.path)
            .load()
            .map_err(HostBootError::Services)?;
        let services = ServicesState::from_config(profile, services_config)
            .map_err(HostBootError::Services)?;

        let database = connect_database(profile, &config).await?;
        let instance_id = config.instance_id.clone();
        let log_sink = systemprompt_logging::init_logging(&database, Some(instance_id.clone()));
        let file_storage = crate::storage::init_file_storage(
            &profile.storage,
            &app_paths,
            &instance_id,
            &|name| secrets.get(name).map(str::to_owned),
        )
        .await?;
        let cache_epochs = CacheEpochs::new();
        let store: Arc<dyn SecretStore> = Arc::<GrantedSecrets>::clone(&secrets);
        let (_policy, authz_hook, market) =
            authz_stage(profile, store, &database, &services, None, &cache_epochs)?;
        let (extensions, _schema, governance) = init_governed_extensions(
            profile,
            None,
            SchemaPolicy::default(),
            MigrationConfig::default(),
            &database,
        )
        .await?;
        let (event_router, _bridge, listener_pool) =
            event_plane(None, &database, &config, &cache_epochs);
        let background_tasks = BackgroundTasks::default();

        #[cfg(feature = "notifications")]
        let notification_sink = crate::notification_sink(&database, &instance_id);
        #[cfg(not(feature = "notifications"))]
        let notification_sink: systemprompt_extension::DynNotificationSink =
            Arc::new(crate::documents::NotificationsUnavailableSink);
        #[cfg(not(feature = "notifications"))]
        drop(listener_pool);
        #[cfg(feature = "brain")]
        let brain_core = crate::BrainCore::new(&crate::BrainInputs {
            database: &database,
            instance_id: &instance_id,
            listener_pool: &listener_pool,
            notifications: &notification_sink,
            profile: &profile.brain,
        });
        #[cfg(feature = "brain")]
        let brain_sink = crate::brain_sink(brain_core.as_ref());
        #[cfg(not(feature = "brain"))]
        let brain_sink: systemprompt_extension::DynBrainSink =
            Arc::new(crate::documents::BrainUnavailableSink);

        let documents = build_documents(&DocumentsInputs {
            services_root: &services_root.path,
            database: &database,
            file_storage: &file_storage,
            authz_hook: &authz_hook,
            subjects: &market.factory.subjects,
            event_router: &event_router,
            files_config: &files_config,
            background_tasks: &background_tasks,
            extensions: &extensions,
            notifications: &notification_sink,
            brain: &brain_sink,
            audiences: &ExtensionAudiences::default(),
        })?;
        #[cfg(feature = "notifications")]
        let notifications = Arc::clone(
            &crate::core_notifications(&crate::NotificationsInputs {
                database: &database,
                event_router: &event_router,
                sink: &notification_sink,
                listener_pool: &listener_pool,
                documents: &documents,
                profile: &profile.notifications,
            })
            .service,
        );

        Ok(Self {
            artifact_ingest: build_artifact_ingest(&database, &governance.get()),
            tool_call_intents: Arc::new(systemprompt_ai::repository::AiRequestRepository::new(
                &database,
            )),
            mcp_sessions: Arc::new(McpSessionRepository::new(&database)),
            user_repository: Arc::new(UserRepository::new(&database)),
            subject_providers: market.subject_providers,
            profile: Arc::clone(&loaded),
            services_root,
            config,
            app_paths,
            files_config,
            database,
            services,
            secrets,
            verification,
            authz_hook,
            documents,
            #[cfg(feature = "notifications")]
            notifications,
            background_tasks,
            log_sink,
        })
    }
}

fn verification_keyring(
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<VerificationKeyring, HostBootError> {
    let raw = lookup(VERIFICATION_KEYS_ENV).ok_or(HostBootError::VerificationKeysMissing)?;
    let keys: TokenVerificationKeys =
        serde_json::from_str(&raw).map_err(HostBootError::VerificationKeysInvalid)?;
    VerificationKeyring::new(keys).map_err(HostBootError::VerificationKey)
}
