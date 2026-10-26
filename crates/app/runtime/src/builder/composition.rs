//! Runtime data-plane and repository composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::authz::PluginInputs;
use super::{Arc, DataPlane, RuntimeResult, UserService, assembly};
use crate::context::Plugins;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_security::at_rest::AtRestCipher;

pub(super) fn build_data_plane(
    database: Arc<systemprompt_database::Database>,
    analytics_service: Arc<systemprompt_analytics::AnalyticsService>,
    fingerprint_repo: Option<Arc<systemprompt_analytics::FingerprintRepository>>,
    user_service: Arc<UserService>,
    repositories: RepositoryBundles,
) -> DataPlane {
    DataPlane {
        database,
        analytics_service,
        fingerprint_repo,
        user_service: Some(user_service),
        a2a_repositories: repositories.a2a,
        content_repositories: repositories.content,
        oauth_repositories: repositories.oauth,
        user_repository: repositories.users,
        service_repository: repositories.services,
        coordination: repositories.coordination,
        ai_repositories: repositories.ai,
        analytics_repositories: repositories.analytics,
        file_repository: repositories.files,
        mcp_session_repository: repositories.mcp_sessions,
        managed_repository: repositories.managed,
    }
}

pub(super) async fn ensure_legacy_context(
    repositories: &RepositoryBundles,
    system_admin: &systemprompt_manifest::services::SystemAdmin,
) -> RuntimeResult<()> {
    repositories
        .a2a
        .contexts
        .ensure_legacy_context(system_admin.id())
        .await?;
    Ok(())
}

pub(super) struct RepositoryBundles {
    pub(super) a2a: Arc<systemprompt_agent::repository::A2ARepositories>,
    content: Arc<systemprompt_content::repository::ContentRepositories>,
    oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    services: Arc<systemprompt_database::ServiceRepository>,
    coordination: Arc<systemprompt_database::CoordinationRepositories>,
    pub(super) ai: Arc<systemprompt_ai::repository::AiRepositories>,
    analytics: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    files: Arc<systemprompt_files::FileRepository>,
    mcp_sessions: Arc<systemprompt_mcp::repository::McpSessionRepository>,
    managed: Arc<systemprompt_marketplace::managed::ManagedRepository>,
}

impl RepositoryBundles {
    pub(super) fn install_organization_resolver(
        &mut self,
        owner: &systemprompt_identifiers::UserId,
    ) {
        let resolver = Arc::new(
            systemprompt_marketplace::managed::OrganizationSkillResolver::new(
                self.managed.as_ref().clone(),
                owner.clone(),
            ),
        );
        self.a2a = Arc::new(
            self.a2a
                .as_ref()
                .clone()
                .with_managed_skill_resolver(resolver),
        );
    }
}

pub(super) fn build_repositories(
    database: &DbPool,
    analytics: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    instance_id: systemprompt_identifiers::InstanceId,
    publication: systemprompt_marketplace::managed::PublicationPolicy,
    secrets: &systemprompt_manifest::secrets::Secrets,
) -> RuntimeResult<RepositoryBundles> {
    let cipher = AtRestCipher::from_secrets(secrets)?;
    let pepper: Arc<str> = Arc::from(secrets.oauth_at_rest_pepper.as_str());
    let session_store = Arc::clone(&analytics.session_store);
    let session_usage: systemprompt_traits::DynSessionUsageCounters = session_store;
    let managed = Arc::new(
        systemprompt_marketplace::managed::ManagedRepository::new(database)
            .with_publication_policy(publication),
    );
    let ai = Arc::new(systemprompt_ai::repository::AiRepositories::new(
        database, cipher,
    ));
    let tool_executions: systemprompt_traits::DynToolExecutionLookup = Arc::new(
        systemprompt_mcp::repository::ToolUsageRepository::new(database),
    );
    let managed_resolver: systemprompt_traits::DynManagedSkillResolver = Arc::new(
        systemprompt_marketplace::managed::ManagedResourceResolver::new(managed.as_ref().clone()),
    );
    Ok(RepositoryBundles {
        a2a: Arc::new(systemprompt_agent::repository::A2ARepositories::new(
            database,
            systemprompt_agent::repository::A2aDependencies {
                session_usage,
                instance_id: instance_id.clone(),
                managed_skills: managed_resolver,
                tool_executions,
            },
        )),
        content: Arc::new(systemprompt_content::repository::ContentRepositories::new(
            database,
        )),
        oauth: Arc::new(systemprompt_oauth::repository::OAuthRepositories::new(
            database, pepper,
        )),
        users: Arc::new(systemprompt_users::UserRepository::new(database)),
        services: Arc::new(systemprompt_database::ServiceRepository::new(
            database,
            instance_id.clone(),
        )),
        coordination: Arc::new(systemprompt_database::CoordinationRepositories::new(
            database,
            instance_id,
        )),
        ai,
        analytics,
        files: Arc::new(systemprompt_files::FileRepository::new(database)),
        mcp_sessions: Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(
            database,
        )),
        managed,
    })
}

pub fn owner_reassignments(database: &DbPool) -> Vec<systemprompt_traits::DynOwnerReassignment> {
    let logging = systemprompt_logging::LoggingOwnerReassignment::new(database);
    let events =
        systemprompt_events::EventsOwnerReassignment::new(database.write_pool().as_ref().clone());
    vec![
        Arc::new(systemprompt_agent::repository::AgentOwnerReassignment::new(
            database,
        )),
        Arc::new(systemprompt_mcp::repository::McpOwnerReassignment::new(
            database,
        )),
        Arc::new(systemprompt_ai::repository::AiOwnerReassignment::new(
            database,
        )),
        Arc::new(systemprompt_analytics::repository::AnalyticsOwnerReassignment::new(database)),
        Arc::new(systemprompt_files::FilesOwnerReassignment::new(database)),
        Arc::new(systemprompt_content::repository::ContentOwnerReassignment::new(database)),
        Arc::new(logging),
        Arc::new(events),
    ]
}

pub(super) fn new_plugins(
    extension_registry: Arc<ExtensionRegistry>,
    mcp_registry: RegistryService,
    marketplace_filter: Option<Arc<dyn MarketplaceFilter>>,
    inputs: &PluginInputs,
) -> Plugins {
    let factory_ctx = &inputs.factory;
    Plugins {
        extension_registry,
        mcp_registry,
        marketplace_filter: marketplace_filter
            .unwrap_or_else(|| assembly::build_marketplace_filter(factory_ctx)),
        marketplace_cache: Arc::default(),
        subject_providers: inputs.subject_providers.clone(),
        subjects: factory_ctx.subjects.clone(),
    }
}

pub(super) fn build_artifact_ingest(
    database: &DbPool,
    governance: &systemprompt_governance::policy::GovernanceEngine,
) -> Arc<systemprompt_mcp::ArtifactIngest> {
    let scanner = governance.secret_scanner().map(|s| Arc::new(s.clone()));
    Arc::new(systemprompt_mcp::ArtifactIngest::from_db(database, scanner))
}

pub(super) async fn build_domain_layer(
    boot: &BootContext,
    database: &DbPool,
    analytics_repositories: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    policy: systemprompt_marketplace::managed::PublicationPolicy,
    services: &ServicesState,
) -> RuntimeResult<(
    RepositoryBundles,
    Arc<UserService>,
    Arc<systemprompt_manifest::SystemAdmin>,
    RegistryService,
)> {
    let config = boot.config()?;
    let mut repositories = build_repositories(
        database,
        analytics_repositories,
        config.instance_id.clone(),
        policy,
        &boot.secrets().get(),
    )?;
    let user_service = Arc::new(
        UserService::new(Arc::clone(&repositories.users))
            .with_owner_reassignments(owner_reassignments(database)),
    );
    let system_admin = assembly::load_system_admin(config, &user_service).await?;
    repositories.install_organization_resolver(system_admin.id());
    let mcp_registry = RegistryService::new(
        system_admin.id().clone(),
        services.view(),
        Arc::clone(config),
    );
    ensure_legacy_context(&repositories, &system_admin).await?;
    Ok((repositories, user_service, system_admin, mcp_registry))
}

pub(super) fn new_spawner() -> RuntimeResult<Arc<systemprompt_loader::subprocess::Spawner>> {
    systemprompt_loader::subprocess::Spawner::new()
        .map(Arc::new)
        .map_err(crate::error::RuntimeError::Spawner)
}
