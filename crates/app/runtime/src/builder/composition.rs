//! Runtime data-plane and repository composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::authz::PluginInputs;
use super::{Arc, DataPlane, RuntimeResult, assembly};
use crate::context::Plugins;
use systemprompt_database::DbPool;
use systemprompt_loader::BootContext;

pub(super) fn build_data_plane(repositories: RepositoryBundles) -> DataPlane {
    DataPlane {
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
    services: Arc<systemprompt_database::ServiceRepository>,
    coordination: Arc<systemprompt_database::CoordinationRepositories>,
}

pub(super) struct SeededRepositories {
    pub(super) services: Arc<systemprompt_database::ServiceRepository>,
    pub(super) coordination: Arc<systemprompt_database::CoordinationRepositories>,
}

pub(super) fn seeded_repositories(
    database: &DbPool,
    instance_id: &systemprompt_identifiers::InstanceId,
) -> SeededRepositories {
    SeededRepositories {
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

pub(super) fn build_repositories(seeded: &SeededRepositories) -> RepositoryBundles {
    RepositoryBundles {
        services: Arc::clone(&seeded.services),
        coordination: Arc::clone(&seeded.coordination),
    }
}

pub(super) fn new_plugins(inputs: &PluginInputs) -> Plugins {
    Plugins {
        subject_providers: inputs.subject_providers.clone(),
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
    admitted: &systemprompt_extension::Admitted,
    database: &Arc<systemprompt_database::Database>,
) -> RuntimeResult<Arc<systemprompt_manifest::SystemAdmin>> {
    let config = boot.config()?;
    let admins = crate::admin_bootstrap(
        admitted,
        Arc::<systemprompt_database::Database>::clone(database),
    )?;
    assembly::load_system_admin(config, &admins).await
}

pub(super) async fn build_domain_layer(
    seeded: &SeededRepositories,
    system_admin: &systemprompt_manifest::SystemAdmin,
    contexts: Option<systemprompt_traits::DynContextMaterializer>,
) -> RuntimeResult<RepositoryBundles> {
    let repositories = build_repositories(seeded);
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
