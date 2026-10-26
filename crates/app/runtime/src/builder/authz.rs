//! The authorization stage of the core boot: the publication policy, the
//! authz hook and the marketplace factory context share one subject source
//! handle, bound once the providers exist.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::Database;
use systemprompt_governance::authz::{
    AuthzHookContext, NullAuditSink, SharedAuthzHook, SubjectProviderSet,
};
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_models::authz::SubjectSourceHandle;
use systemprompt_traits::{CacheEpochs, SecretStore};

use super::core_layer::chain_sources;
use crate::error::RuntimeResult;

pub(super) struct PluginInputs {
    pub(super) factory: systemprompt_marketplace::MarketplaceFactoryContext,
    pub(super) subject_providers: SubjectProviderSet,
}

fn authz_wiring(
    factory: &systemprompt_marketplace::MarketplaceFactoryContext,
    services: &ServicesState,
    epochs: CacheEpochs,
    providers: &SubjectProviderSet,
) -> systemprompt_governance::authz::AuthzWiring {
    systemprompt_governance::authz::AuthzWiring {
        sources: chain_sources(&services.config()),
        epochs,
        services: factory.services.clone(),
        subjects: factory.subjects.clone(),
        providers: providers.clone(),
        secrets: Arc::clone(&factory.secrets),
    }
}

pub(super) fn authz_stage(
    boot: &BootContext,
    database: &Arc<Database>,
    services: &ServicesState,
    hook_override: Option<SharedAuthzHook>,
    epochs: &CacheEpochs,
) -> RuntimeResult<(
    systemprompt_marketplace::managed::PublicationPolicy,
    SharedAuthzHook,
    PluginInputs,
)> {
    let secrets: Arc<dyn SecretStore> =
        Arc::<systemprompt_config::SecretsService>::clone(boot.secrets());
    let factory = systemprompt_marketplace::MarketplaceFactoryContext {
        db: Arc::clone(database),
        services: services.view(),
        subjects: SubjectSourceHandle::default(),
        secrets,
    };
    let providers = SubjectProviderSet::discover(&AuthzHookContext {
        pool: database.pool(),
        sink: Arc::new(NullAuditSink),
        services: factory.services.clone(),
        subjects: factory.subjects.clone(),
        secrets: Arc::clone(&factory.secrets),
    });
    factory.subjects.bind(Arc::new(providers.clone()))?;
    let policy = systemprompt_marketplace::managed::PublicationPolicy::discover(
        &factory,
        services.config().marketplace.clone().unwrap_or_default(),
        providers.clone(),
    )?;
    let authz_hook = systemprompt_governance::authz::build_authz_hook(
        boot.profile().governance.as_ref(),
        database.write_pool(),
        hook_override,
        authz_wiring(&factory, services, epochs.clone(), &providers),
    )?;
    let market = PluginInputs {
        factory,
        subject_providers: providers,
    };
    Ok((policy, authz_hook, market))
}
