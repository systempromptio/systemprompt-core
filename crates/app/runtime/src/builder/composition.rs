//! Runtime data-plane and repository composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Arc, DataPlane, RuntimeResult, UserService};

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
    }
}

pub(super) async fn ensure_legacy_context(
    repositories: &RepositoryBundles,
    system_admin: &systemprompt_models::services::SystemAdmin,
) -> RuntimeResult<()> {
    repositories
        .a2a
        .contexts
        .ensure_legacy_context(system_admin.id())
        .await
        .map_err(|e| crate::error::RuntimeError::Internal(e.to_string()))
}

pub(super) struct RepositoryBundles {
    pub(super) a2a: Arc<systemprompt_agent::repository::A2ARepositories>,
    content: Arc<systemprompt_content::repository::ContentRepositories>,
    oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    services: Arc<systemprompt_database::ServiceRepository>,
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
    database: &systemprompt_database::DbPool,
    analytics: Arc<systemprompt_analytics::repository::AnalyticsRepositories>,
    instance_id: systemprompt_identifiers::InstanceId,
) -> RuntimeResult<RepositoryBundles> {
    let session_usage: systemprompt_traits::DynSessionUsageCounters = analytics.sessions.owner();
    let managed = Arc::new(systemprompt_marketplace::managed::ManagedRepository::new(
        database,
    )?);
    let ai = Arc::new(systemprompt_ai::repository::AiRepositories::new(database)?);
    let tool_executions: systemprompt_traits::DynToolExecutionLookup = Arc::new(
        systemprompt_mcp::repository::ToolUsageRepository::new(database)?,
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
        )?),
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
        ai,
        analytics,
        files: Arc::new(systemprompt_files::FileRepository::new(database)?),
        mcp_sessions: Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(
            database,
        )?),
        managed,
    })
}
