//! The authorization stage of the core boot: the publication policy and the
//! authz hook are assembled from the kernel-built subject providers, authz
//! hooks and marketplace seams, and share one subject source handle bound to
//! those providers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::Database;
use systemprompt_governance::authz::{AuthzWiring, SharedAuthzHook, SubjectProviderSet};
use systemprompt_governance::contract::AuthzHooks;
use systemprompt_loader::ServicesState;
use systemprompt_marketplace::contract::MarketplaceSeams;
use systemprompt_marketplace::managed::PublicationPolicy;
use systemprompt_models::authz::SubjectSourceHandle;
use systemprompt_traits::CacheEpochs;

use super::core_layer::chain_sources;
use crate::error::RuntimeResult;

pub(crate) struct PluginInputs {
    pub(crate) subject_providers: SubjectProviderSet,
    pub(crate) subjects: SubjectSourceHandle,
}

pub(crate) struct AuthzBuilt<'a> {
    pub(crate) providers: &'a SubjectProviderSet,
    pub(crate) hooks: &'a AuthzHooks,
    pub(crate) seams: &'a MarketplaceSeams,
    pub(crate) epochs: &'a CacheEpochs,
}

pub(crate) fn authz_stage(
    profile: &systemprompt_manifest::Profile,
    database: &Arc<Database>,
    services: &ServicesState,
    hook_override: Option<SharedAuthzHook>,
    built: &AuthzBuilt<'_>,
) -> RuntimeResult<(PublicationPolicy, SharedAuthzHook, PluginInputs)> {
    let providers = built.providers.clone();
    let subjects = SubjectSourceHandle::default();
    subjects.bind(Arc::new(providers.clone()))?;
    let policy = PublicationPolicy::from_seams(
        services.config().marketplace.clone().unwrap_or_default(),
        built.seams,
        providers.clone(),
    );
    let authz_hook = systemprompt_governance::authz::build_authz_hook(
        profile.governance.as_ref(),
        database.write_pool(),
        hook_override.or_else(|| built.hooks.hook()),
        AuthzWiring {
            sources: chain_sources(&services.config()),
            epochs: built.epochs.clone(),
            providers: providers.clone(),
        },
    )?;
    let market = PluginInputs {
        subject_providers: providers,
        subjects,
    };
    Ok((policy, authz_hook, market))
}
