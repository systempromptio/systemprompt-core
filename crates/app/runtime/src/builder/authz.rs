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
use systemprompt_loader::ServicesState;
use systemprompt_manifest::Profile;
use systemprompt_models::authz::SubjectSourceHandle;
use systemprompt_traits::CacheEpochs;

use super::core_layer::chain_sources;
use crate::error::RuntimeResult;

pub(super) struct PluginInputs {
    pub(super) factory: systemprompt_marketplace::MarketplaceFactoryContext,
    pub(super) subject_providers: SubjectProviderSet,
}

fn authz_wiring(
    services: &ServicesState,
    epochs: CacheEpochs,
    subjects: &SubjectSourceHandle,
    providers: &SubjectProviderSet,
) -> systemprompt_governance::authz::AuthzWiring {
    systemprompt_governance::authz::AuthzWiring {
        sources: chain_sources(&services.config()),
        epochs,
        services: services.view(),
        subjects: subjects.clone(),
        providers: providers.clone(),
    }
}

pub(super) fn authz_stage(
    profile: &Profile,
    database: &Arc<Database>,
    services: &ServicesState,
    hook_override: Option<SharedAuthzHook>,
    epochs: &CacheEpochs,
) -> RuntimeResult<(
    systemprompt_marketplace::managed::PublicationPolicy,
    SharedAuthzHook,
    PluginInputs,
)> {
    let subjects = SubjectSourceHandle::default();
    let providers = SubjectProviderSet::discover(&AuthzHookContext {
        pool: database.pool(),
        sink: Arc::new(NullAuditSink),
        services: services.view(),
        subjects: subjects.clone(),
    });
    subjects.bind(Arc::new(providers.clone()))?;
    let policy = systemprompt_marketplace::managed::PublicationPolicy::discover(
        database,
        services.config().marketplace.clone().unwrap_or_default(),
        services.view(),
        subjects.clone(),
        providers.clone(),
    )?;
    let authz_hook = systemprompt_governance::authz::build_authz_hook(
        profile.governance.as_ref(),
        database.write_pool(),
        hook_override,
        authz_wiring(services, epochs.clone(), &subjects, &providers),
    )?;
    let market = PluginInputs {
        factory: systemprompt_marketplace::MarketplaceFactoryContext {
            db: Arc::clone(database),
            services: services.view(),
            subjects,
        },
        subject_providers: providers,
    };
    Ok((policy, authz_hook, market))
}
