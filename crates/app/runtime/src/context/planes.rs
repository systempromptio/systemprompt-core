//! The four planes an [`AppContext`](super::AppContext) groups its handles
//! into: data access, configuration, plugins and runtime subsystems.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_agent::repository::A2ARepositories;
use systemprompt_ai::repository::AiRepositories;
use systemprompt_analytics::{FingerprintRepository, GeoIpReader};
use systemprompt_config::paths::AppPaths;
use systemprompt_content::repository::ContentRepositories;
use systemprompt_database::{CoordinationRepositories, DbPool, ServiceRepository};
use systemprompt_events::{EventBridgeHandle, EventRouter};
use systemprompt_files::FilesConfig;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_loader::BootContext;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_models::{ContentConfigRaw, RouteClassifier};
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_security::credential::TokenCache;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{BackgroundTasks, CacheEpochs, ConfigWatch, FileStorage};
use systemprompt_users::{UserRepository, UserService};

use super::ShutdownRequest;

/// The four planes an [`AppContext`](super::AppContext) holds.
#[derive(Debug, Clone)]
pub struct ContextPlanes {
    pub data: DataPlane,
    pub cfg: ConfigPlane,
    pub plugins: Plugins,
    pub subsystems: Subsystems,
}

/// Database pool and the data-access services layered on it.
///
/// `fingerprint_repo` and `user_service` are `None` when the corresponding
/// resource failed to initialise; callers must degrade gracefully.
#[derive(Clone)]
pub struct DataPlane {
    pub database: DbPool,
    pub fingerprint_repo: Option<Arc<FingerprintRepository>>,
    pub user_service: Option<Arc<UserService>>,
    pub a2a_repositories: Arc<A2ARepositories>,
    pub content_repositories: Arc<ContentRepositories>,
    pub oauth_repositories: Arc<OAuthRepositories>,
    pub user_repository: Arc<UserRepository>,
    pub service_repository: Arc<ServiceRepository>,
    pub coordination: Arc<CoordinationRepositories>,
    pub ai_repositories: Arc<AiRepositories>,
    pub mcp_session_repository: Arc<McpSessionRepository>,
}

#[derive(Clone)]
pub struct ConfigPlane {
    pub boot: BootContext,
    pub config: Arc<Config>,
    pub app_paths: Arc<AppPaths>,
    pub files_config: Arc<FilesConfig>,
    pub content_config: Option<Arc<ContentConfigRaw>>,
    pub route_classifier: Arc<RouteClassifier>,
    pub build: systemprompt_models::BuildIdentity,
}

#[derive(Clone)]
pub struct Plugins {
    pub mcp_registry: RegistryService,
    pub subject_providers: systemprompt_governance::authz::SubjectProviderSet,
    pub subjects: systemprompt_models::authz::SubjectSourceHandle,
}

#[derive(Clone)]
pub struct Subsystems {
    pub system_admin: Arc<SystemAdmin>,
    pub logging: LoggingRuntime,
    pub authz_hook: SharedAuthzHook,
    pub signing_keys: SigningKeys,
    pub governance: ConfigWatch<GovernanceEngine>,
    pub services: systemprompt_loader::ServicesState,
    pub cache_epochs: CacheEpochs,
    pub ai_service: Option<Arc<systemprompt_ai::AiService>>,
    pub artifact_ingest: Arc<systemprompt_mcp::ArtifactIngest>,
    pub schema_install: Arc<crate::SchemaState>,
    pub event_bridge: Arc<EventBridgeHandle>,
    pub event_router: EventRouter,
    pub geoip_reader: Option<GeoIpReader>,
    pub file_storage: Arc<dyn FileStorage>,
    pub shutdown: ShutdownRequest,
    pub background_tasks: BackgroundTasks,
    pub spawner: Arc<systemprompt_loader::subprocess::Spawner>,
    pub connector_tokens: Arc<TokenCache>,
    pub documents: Arc<crate::documents::DocumentsRuntime>,
}
