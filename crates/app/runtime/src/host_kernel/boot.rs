//! [`HostKernel::boot`]: the composition stages run over a child's granted
//! environment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_config::{GrantedSecrets, LoadedProfile};
use systemprompt_database::MigrationConfig;
use systemprompt_loader::{ConfigLoader, ServicesSourceBootstrap, ServicesState};
use systemprompt_marketplace::managed::ManagedRepository;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_security::keys::{TokenVerificationKeys, VerificationKeyring};
use systemprompt_traits::{BackgroundTasks, CacheEpochs, DynSessionUsageCounters, SecretStore};
use systemprompt_users::{SessionRepository, UserRepository};

use crate::builder::assembly::load_system_admin;
use crate::builder::authz::authz_stage;
use crate::builder::composition::{a2a_repositories, build_artifact_ingest, organization_skills};
use crate::builder::core_layer::{SchemaPolicy, init_files_config};
use crate::builder::database::connect_database;
use crate::builder::documents::{DocumentsInputs, build_documents};
use crate::builder::events::event_plane;
use crate::builder::governance::init_governed_extensions;
#[cfg(feature = "notifications")]
use crate::builder::notifications::NotificationsCore;
use crate::documents::ExtensionAudiences;
use crate::error::RuntimeError;

use super::{HostBootError, HostKernel, HostParts, PROFILE_ENV};

impl HostKernel {
    pub async fn from_process_env() -> Result<(HostParts, Self), HostBootError> {
        Self::boot(|name| std::env::var(name).ok()).await
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the one assembly sequence of a child host; splitting it threads a dozen handles through helper signatures"
    )]
    pub async fn boot(
        lookup: impl Fn(&str) -> Option<String> + Send + Sync,
    ) -> Result<(HostParts, Self), HostBootError> {
        let profile_path = lookup(PROFILE_ENV).ok_or(HostBootError::ProfileMissing)?;
        let loaded = LoadedProfile::from_path(Path::new(&profile_path))
            .map_err(|e| HostBootError::Profile(Box::new(e)))?;
        let profile = loaded.profile();
        let secrets = Arc::new(
            GrantedSecrets::from_lookup(&lookup)
                .map_err(|e| HostBootError::Secrets(Box::new(e)))?,
        );
        let verification = verification_keyring(&lookup)?;

        let services_root = ServicesSourceBootstrap::read_composed(profile)
            .map_err(|e| HostBootError::ServicesRoot(Box::new(e)))?;
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
            .map_err(|e| HostBootError::Config(Box::new(e)))?,
        );
        let files_config = init_files_config(profile, &app_paths)?;
        let services_config = ConfigLoader::for_profile(profile, &services_root.path)
            .load()
            .map_err(|e| HostBootError::Services(Box::new(e)))?;
        let services = ServicesState::from_config(profile, services_config)
            .map_err(|e| HostBootError::Services(Box::new(e)))?;

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
        let notifications = NotificationsCore::new(&crate::NotificationsInputs {
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

        let managed =
            ManagedRepository::new(&database).with_publication_policy(publication.clone());
        let session_usage: DynSessionUsageCounters = Arc::new(SessionRepository::new(&database));
        let a2a = Arc::new(organization_skills(
            &a2a_repositories(&database, instance_id.clone(), &managed, session_usage),
            &managed,
            &system_owner,
        ));

        let kernel = Self {
            market_state: market.state,
            extensions: Arc::clone(&extensions),
            disabled_extensions: profile.extensions.disabled.clone(),
            #[cfg(feature = "notifications")]
            audiences,
            #[cfg(feature = "notifications")]
            notifications,
        };
        let parts = HostParts {
            artifact_ingest: build_artifact_ingest(&database, &governance.get()),
            tool_call_intents: Arc::new(systemprompt_ai::repository::AiRequestRepository::new(
                &database,
            )),
            mcp_sessions: Arc::new(McpSessionRepository::new(&database)),
            tokens: Arc::new(VerifyingTokenAuthority::new(
                verification.clone(),
                config.jwt_issuer.clone(),
            )),
            subjects: market.factory.subjects.clone(),
            subject_providers: market.subject_providers,
            user_repository,
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
            background_tasks,
            log_sink,
            file_storage,
            extensions,
            system_owner,
            a2a,
        };
        Ok((parts, kernel))
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
