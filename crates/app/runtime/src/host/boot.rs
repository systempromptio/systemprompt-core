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
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_security::keys::{TokenVerificationKeys, VerificationKeyring};
use systemprompt_traits::{BackgroundTasks, CacheEpochs, SecretStore};
use systemprompt_users::UserRepository;

use super::HostContext;
use super::extension_context::ExtensionHandles;
use crate::builder::assembly::load_system_admin;
use crate::builder::authz::authz_stage;
use crate::builder::composition::build_artifact_ingest;
use crate::builder::core_layer::{SchemaPolicy, init_files_config};
use crate::builder::database::connect_database;
use crate::builder::documents::{DocumentsInputs, build_documents};
use crate::builder::events::event_plane;
use crate::builder::governance::init_governed_extensions;
#[cfg(feature = "notifications")]
use crate::builder::notifications::{ExtensionSeams, NotificationsCore};
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
    #[error("the services composition the API resolved could not be read")]
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

        let services_root =
            ServicesSourceBootstrap::read_composed(profile).map_err(HostBootError::ServicesRoot)?;
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
        let (publication, authz_hook, market) =
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

        let audiences = ExtensionAudiences::default();
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
            disabled_extensions: &profile.extensions.disabled,
            notifications: &notification_sink,
            brain: &brain_sink,
            audiences: &audiences,
        })?;
        #[cfg(feature = "notifications")]
        let notifications_core = NotificationsCore::new(&crate::NotificationsInputs {
            database: &database,
            event_router: &event_router,
            sink: &notification_sink,
            listener_pool: &listener_pool,
            documents: &documents,
            profile: &profile.notifications,
            escalation: systemprompt_notifications::service::EscalationDelivery::Consumer,
        });
        let user_repository = Arc::new(UserRepository::new(&database));
        let system_owner = load_system_admin(
            &config,
            &Arc::new(systemprompt_users::UserService::new(Arc::clone(
                &user_repository,
            ))),
        )
        .await?
        .id()
        .clone();
        let extension = ExtensionHandles {
            tokens: Arc::new(VerifyingTokenAuthority::new(
                verification.clone(),
                config.jwt_issuer.clone(),
            )),
            file_storage,
            extensions,
            state: systemprompt_provider_contracts::Dependencies::new(),
            subjects: market.factory.subjects.clone(),
            provider_tokens: Arc::default(),
            system_owner,
        };

        let host = Self {
            artifact_ingest: build_artifact_ingest(&database, &governance.get()),
            tool_call_intents: Arc::new(systemprompt_ai::repository::AiRequestRepository::new(
                &database,
            )),
            mcp_sessions: Arc::new(McpSessionRepository::new(&database)),
            user_repository,
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
            publication,
            documents,
            #[cfg(feature = "notifications")]
            notifications: Arc::clone(
                &notifications_core
                    .assemble(ExtensionSeams::default())
                    .service,
            ),
            background_tasks,
            log_sink,
            extension,
        };
        let host = {
            let mut host = host;
            let state = host
                .extension
                .extensions
                .build_state(&host, host.kernel_state(), &profile.extensions.disabled)
                .map_err(RuntimeError::from)?;
            market
                .state
                .bind(state.clone())
                .map_err(RuntimeError::from)?;
            host.extension.state = state;
            host
        };
        #[cfg(feature = "notifications")]
        let host = {
            let mut host = host;
            let seams = ExtensionSeams::discover(&host);
            audiences.bind(seams.audiences.clone());
            host.notifications = Arc::clone(&notifications_core.assemble(seams).service);
            host
        };
        Ok(host)
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
