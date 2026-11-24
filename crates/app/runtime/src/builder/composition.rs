//! Runtime data-plane and repository composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::authz::PluginInputs;
use super::{Arc, DataPlane, RuntimeResult, UserService, assembly};
use crate::context::Plugins;
use systemprompt_database::DbPool;
use systemprompt_loader::BootContext;

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
    }
}

pub(super) async fn ensure_legacy_context(
    contexts: Option<systemprompt_traits::DynContextMaterializer>,
    system_admin: &systemprompt_manifest::services::SystemAdmin,
) -> RuntimeResult<()> {
    let Some(contexts) = contexts else {
        tracing::info!("no context materializer is provided; the legacy context is not ensured");
        return Ok(());
    };
    contexts
        .ensure_context(systemprompt_traits::EnsureContextParams {
            context_id: &systemprompt_identifiers::ContextId::legacy_context_row(),
            user_id: system_admin.id(),
            session_id: None,
            name: LEGACY_CONTEXT_NAME,
            kind: systemprompt_models::api::ContextKind::Legacy.as_str(),
        })
        .await
        .map_err(crate::error::RuntimeError::LegacyContext)
}

const LEGACY_CONTEXT_NAME: &str = "Legacy (pre-context)";

pub(super) struct RepositoryBundles {
    content: Arc<systemprompt_content::repository::ContentRepositories>,
    oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    services: Arc<systemprompt_database::ServiceRepository>,
    coordination: Arc<systemprompt_database::CoordinationRepositories>,
}

pub(super) struct SeededRepositories {
    pub(super) oauth: Arc<systemprompt_oauth::repository::OAuthRepositories>,
    pub(super) users: Arc<systemprompt_users::UserRepository>,
    pub(super) services: Arc<systemprompt_database::ServiceRepository>,
    pub(super) coordination: Arc<systemprompt_database::CoordinationRepositories>,
}

pub(super) fn seeded_repositories(
    database: &DbPool,
    secrets: &systemprompt_manifest::secrets::Secrets,
    instance_id: &systemprompt_identifiers::InstanceId,
) -> SeededRepositories {
    let pepper: Arc<str> = Arc::from(secrets.oauth_at_rest_pepper.as_str());
    SeededRepositories {
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
            instance_id.clone(),
        )),
    }
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
    }
}

pub(super) fn new_plugins(inputs: &PluginInputs) -> Plugins {
    Plugins {
        subject_providers: inputs.subject_providers.clone(),
        subjects: inputs.subjects.clone(),
    }
}

pub(crate) fn bind_secret_scanner(
    governance: &systemprompt_governance::policy::GovernanceEngine,
    binder: systemprompt_governance::policy::secrets::SecretScannerBinder,
) -> RuntimeResult<()> {
    if let Some(configured) = governance.secret_scanner() {
        binder.bind(Arc::new(configured.clone()))?;
    }
    Ok(())
}

pub(super) async fn load_identity(
    boot: &BootContext,
    users: &Arc<systemprompt_users::UserRepository>,
) -> RuntimeResult<Arc<systemprompt_manifest::SystemAdmin>> {
    let config = boot.config()?;
    let lookup = Arc::new(UserService::new(Arc::clone(users)));
    assembly::load_system_admin(config, &lookup).await
}

pub(super) async fn build_domain_layer(
    database: &DbPool,
    seeded: &SeededRepositories,
    system_admin: &systemprompt_manifest::SystemAdmin,
    contexts: Option<systemprompt_traits::DynContextMaterializer>,
) -> RuntimeResult<RepositoryBundles> {
    let repositories = build_repositories(database, seeded);
    ensure_legacy_context(contexts, system_admin).await?;
    Ok(repositories)
}

pub(super) fn new_spawner() -> RuntimeResult<Arc<systemprompt_loader::subprocess::Spawner>> {
    systemprompt_loader::subprocess::Spawner::new()
        .map(Arc::new)
        .map_err(crate::error::RuntimeError::Spawner)
}

pub(crate) fn catalog_roles(
    admitted: &systemprompt_extension::Admitted,
) -> systemprompt_models::authz::CatalogRoleNames {
    systemprompt_models::authz::CatalogRoleNames::new(
        admitted
            .built()
            .members()
            .flat_map(|member| member.sink.roles())
            .map(|role| role.name.clone())
            .collect(),
    )
}
