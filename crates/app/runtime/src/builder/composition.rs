//! Runtime data-plane and repository composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    Arc, DataPlane, OnceLock, RuntimeResult, SharedAuthzHook, ShutdownRequest, Subsystems,
    UserService,
};

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
        ai_repositories: repositories.ai,
        analytics_repositories: repositories.analytics,
        file_repository: repositories.files,
        mcp_session_repository: repositories.mcp_sessions,
        managed_repository: repositories.managed,
        evaluation_repositories: repositories.evaluation,
    }
}

pub(super) fn build_subsystems(
    system_admin: Arc<systemprompt_models::services::SystemAdmin>,
    authz_hook: SharedAuthzHook,
    geoip_reader: Option<systemprompt_analytics::GeoIpReader>,
    file_storage: Arc<dyn systemprompt_traits::FileStorage>,
    shutdown: ShutdownRequest,
) -> Subsystems {
    Subsystems {
        system_admin,
        authz_hook,
        event_bridge: Arc::new(OnceLock::new()),
        geoip_reader,
        file_storage,
        shutdown,
    }
}

pub(super) async fn ensure_legacy_context(
    repositories: &RepositoryBundles,
    system_admin: &systemprompt_models::services::SystemAdmin,
) -> RuntimeResult<()> {
    repositories
        .a2a
        .contexts
        .ensure_context(
            &systemprompt_traits::EnsureContextParams {
                context_id: &systemprompt_identifiers::ContextId::legacy(),
                user_id: system_admin.id(),
                session_id: None,
                name: "Legacy (pre-context)",
                kind: systemprompt_models::ContextKind::Legacy.as_str(),
            },
            systemprompt_models::ContextKind::Legacy,
        )
        .await
        .map_err(|e| crate::error::RuntimeError::Internal(e.to_string()))
}

pub(super) struct RepositoryBundles {
    a2a: Arc<systemprompt_agent::repository::A2ARepositories>,
    content: Arc<systemprompt_content::repository::ContentRepositories>,
    oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    services: Arc<systemprompt_database::ServiceRepository>,
    ai: Arc<systemprompt_ai::repository::AiRepositories>,
    analytics: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    files: Arc<systemprompt_files::FileRepository>,
    mcp_sessions: Arc<systemprompt_mcp::repository::McpSessionRepository>,
    managed: Arc<systemprompt_marketplace::managed::ManagedRepository>,
    evaluation: Arc<systemprompt_evaluation::repository::experiments::EvaluationRepositories>,
}

impl RepositoryBundles {
    pub(super) fn bind_organizational_owner(&mut self, owner: &systemprompt_identifiers::UserId) {
        let resolver = systemprompt_marketplace::managed::ManagedResourceResolver::new(
            self.managed.as_ref().clone(),
        )
        .with_organizational_owner(owner.clone());
        self.a2a = Arc::new(
            self.a2a
                .as_ref()
                .clone()
                .with_managed_skill_resolver(Arc::new(resolver)),
        );
    }
}

pub(super) fn build_repositories(
    database: &systemprompt_database::DbPool,
    analytics: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    instance_id: systemprompt_identifiers::InstanceId,
) -> RuntimeResult<RepositoryBundles> {
    let session_usage: systemprompt_traits::DynSessionUsageCounters =
        Arc::new(analytics.sessions.clone());
    let pool = database
        .pool_arc()
        .map_err(|error| crate::error::RuntimeError::Internal(error.to_string()))?;
    let managed = Arc::new(systemprompt_marketplace::managed::ManagedRepository::new(
        pool.as_ref().clone(),
    ));
    let evaluation = Arc::new(
        systemprompt_evaluation::repository::experiments::EvaluationRepositories::new(
            pool.as_ref(),
        ),
    );
    let managed_resolver: systemprompt_traits::DynManagedSkillResolver = Arc::new(
        systemprompt_marketplace::managed::ManagedResourceResolver::new(managed.as_ref().clone()),
    );
    Ok(RepositoryBundles {
        a2a: Arc::new(
            systemprompt_agent::repository::A2ARepositories::new(
                database,
                session_usage,
                instance_id.clone(),
            )?
            .with_managed_skill_resolver(managed_resolver),
        ),
        content: Arc::new(systemprompt_content::repository::ContentRepositories::new(
            database,
        )?),
        oauth: Arc::new(systemprompt_oauth::repository::OAuthRepositories::new(
            database,
        )?),
        users: Arc::new(systemprompt_users::UserRepository::new(database)?),
        services: Arc::new(systemprompt_database::ServiceRepository::new(
            database,
            instance_id,
        )?),
        ai: Arc::new(systemprompt_ai::repository::AiRepositories::new(database)?),
        analytics,
        files: Arc::new(systemprompt_files::FileRepository::new(database)?),
        mcp_sessions: Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(
            database,
        )?),
        managed,
        evaluation,
    })
}
