//! Runtime data-plane and repository composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::authz::PluginInputs;
use super::{Arc, DataPlane, RuntimeResult, UserService, assembly};
use crate::context::Plugins;
use systemprompt_database::DbPool;
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_security::at_rest::AtRestCipher;

pub(super) fn build_data_plane(
    database: Arc<systemprompt_database::Database>,
    fingerprint_repo: Option<Arc<systemprompt_analytics::FingerprintRepository>>,
    user_service: Arc<UserService>,
    repositories: RepositoryBundles,
) -> DataPlane {
    DataPlane {
        database,
        fingerprint_repo,
        user_service: Some(user_service),
        content_repositories: repositories.content,
        oauth_repositories: repositories.oauth,
        user_repository: repositories.users,
        service_repository: repositories.services,
        coordination: repositories.coordination,
        ai_repositories: repositories.ai,
        mcp_session_repository: repositories.mcp_sessions,
    }
}

pub(super) async fn ensure_legacy_context(
    database: &DbPool,
    system_admin: &systemprompt_manifest::services::SystemAdmin,
) -> RuntimeResult<()> {
    systemprompt_agent::repository::ContextRepository::new(database)
        .ensure_legacy_context(system_admin.id())
        .await?;
    Ok(())
}

pub(super) struct RepositoryBundles {
    content: Arc<systemprompt_content::repository::ContentRepositories>,
    oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    services: Arc<systemprompt_database::ServiceRepository>,
    coordination: Arc<systemprompt_database::CoordinationRepositories>,
    pub(super) ai: Arc<systemprompt_ai::repository::AiRepositories>,
    mcp_sessions: Arc<systemprompt_mcp::repository::McpSessionRepository>,
}

pub(super) struct SeededRepositories {
    pub(super) oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) ai: Arc<systemprompt_ai::repository::AiRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    pub(super) services: Arc<systemprompt_database::ServiceRepository>,
    pub(super) coordination: Arc<systemprompt_database::CoordinationRepositories>,
}

pub(super) fn seeded_repositories(
    database: &DbPool,
    secrets: &systemprompt_manifest::secrets::Secrets,
    instance_id: &systemprompt_identifiers::InstanceId,
) -> RuntimeResult<SeededRepositories> {
    let cipher = AtRestCipher::from_secrets(secrets)?;
    let pepper: Arc<str> = Arc::from(secrets.oauth_at_rest_pepper.as_str());
    Ok(SeededRepositories {
        oauth: Arc::new(systemprompt_oauth::repository::OAuthRepositories::new(
            database, pepper,
        )),
        ai: Arc::new(systemprompt_ai::repository::AiRepositories::new(
            database, cipher,
        )),
        users: Arc::new(systemprompt_users::UserRepository::new(database)),
        services: Arc::new(systemprompt_database::ServiceRepository::new(
            database,
            instance_id.clone(),
        )),
        coordination: Arc::new(systemprompt_database::CoordinationRepositories::new(
            database,
            instance_id.clone(),
        )),
    })
}

pub(super) fn build_repositories(
    database: &DbPool,
    seeded: &SeededRepositories,
) -> RepositoryBundles {
    RepositoryBundles {
        content: Arc::new(systemprompt_content::repository::ContentRepositories::new(
            database,
        )),
        oauth: Arc::clone(&seeded.oauth),
        users: Arc::clone(&seeded.users),
        services: Arc::clone(&seeded.services),
        coordination: Arc::clone(&seeded.coordination),
        ai: Arc::clone(&seeded.ai),
        mcp_sessions: Arc::new(systemprompt_mcp::repository::McpSessionRepository::new(
            database,
        )),
    }
}

pub(super) fn new_plugins(mcp_registry: RegistryService, inputs: &PluginInputs) -> Plugins {
    Plugins {
        mcp_registry,
        subject_providers: inputs.subject_providers.clone(),
        subjects: inputs.subjects.clone(),
    }
}

pub(crate) fn build_artifact_ingest(
    database: &DbPool,
    governance: &systemprompt_governance::policy::GovernanceEngine,
    scanner: systemprompt_governance::policy::secrets::SecretScannerCell,
) -> Arc<systemprompt_mcp::ArtifactIngest> {
    if let Some(configured) = governance.secret_scanner()
        && scanner.bind(Arc::new(configured.clone())).is_err()
    {
        tracing::debug!("the secret scanner cell was bound before this host loaded governance");
    }
    Arc::new(systemprompt_mcp::ArtifactIngest::from_db(database, scanner))
}

pub(super) async fn load_identity(
    boot: &BootContext,
    users: &Arc<systemprompt_users::UserRepository>,
    services: &ServicesState,
) -> RuntimeResult<(Arc<systemprompt_manifest::SystemAdmin>, RegistryService)> {
    let config = boot.config()?;
    let lookup = Arc::new(UserService::new(Arc::clone(users)));
    let system_admin = assembly::load_system_admin(config, &lookup).await?;
    let mcp_registry = RegistryService::new(
        system_admin.id().clone(),
        services.view(),
        Arc::clone(config),
    );
    Ok((system_admin, mcp_registry))
}

pub(super) async fn build_domain_layer(
    database: &DbPool,
    seeded: &SeededRepositories,
    system_admin: &systemprompt_manifest::SystemAdmin,
) -> RuntimeResult<RepositoryBundles> {
    let repositories = build_repositories(database, seeded);
    ensure_legacy_context(database, system_admin).await?;
    Ok(repositories)
}

pub(super) fn new_spawner() -> RuntimeResult<Arc<systemprompt_loader::subprocess::Spawner>> {
    systemprompt_loader::subprocess::Spawner::new()
        .map(Arc::new)
        .map_err(crate::error::RuntimeError::Spawner)
}
