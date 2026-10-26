//! Compile-time registration of SSO providers and sign-in audit sinks.
//!
//! A downstream crate submits a factory with [`register_sso_provider!`] or
//! [`register_auth_event_sink!`]; [`super::SsoDriver::discover`] runs every
//! factory once at a composition root. A provider factory returning `None`
//! means "configured off", so a provider can stay linked but disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_traits::SecretStore;
use systemprompt_traits::sso::{AuthEventSink, SsoProvider};

/// What a factory may read while building its provider or sink.
#[derive(Clone)]
pub struct SsoHookContext {
    pub db: DbPool,
    pub config: Arc<Config>,
    pub services: ServicesView,
    pub services_root: ActiveServicesRoot,
    pub secrets: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for SsoHookContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoHookContext")
            .field("api_external_url", &self.config.api_external_url)
            .finish_non_exhaustive()
    }
}

/// One inventory submission per
/// [`register_sso_provider!`](crate::register_sso_provider) call.
#[derive(Debug, Clone, Copy)]
pub struct SsoProviderRegistration {
    pub factory: fn(&SsoHookContext) -> Option<Arc<dyn SsoProvider>>,
}

inventory::collect!(SsoProviderRegistration);

/// One inventory submission per
/// [`register_auth_event_sink!`](crate::register_auth_event_sink) call.
#[derive(Debug, Clone, Copy)]
pub struct AuthEventSinkRegistration {
    pub factory: fn(&SsoHookContext) -> Arc<dyn AuthEventSink>,
}

inventory::collect!(AuthEventSinkRegistration);

pub(super) fn discover_providers(ctx: &SsoHookContext) -> Vec<Arc<dyn SsoProvider>> {
    let mut providers: Vec<Arc<dyn SsoProvider>> = Vec::new();
    for registration in inventory::iter::<SsoProviderRegistration>() {
        let Some(provider) = (registration.factory)(ctx) else {
            continue;
        };
        if providers.iter().any(|p| p.id() == provider.id()) {
            tracing::error!(
                provider_id = provider.id(),
                "duplicate SSO provider id; keeping the first registration"
            );
            continue;
        }
        providers.push(provider);
    }
    providers
}

pub(super) fn discover_sinks(ctx: &SsoHookContext) -> Vec<Arc<dyn AuthEventSink>> {
    inventory::iter::<AuthEventSinkRegistration>()
        .map(|registration| (registration.factory)(ctx))
        .collect()
}

#[macro_export]
macro_rules! register_sso_provider {
    ($factory:expr) => {
        $crate::__inventory::submit! {
            $crate::sso::SsoProviderRegistration {
                factory: $factory,
            }
        }
    };
}

#[macro_export]
macro_rules! register_auth_event_sink {
    ($factory:expr) => {
        $crate::__inventory::submit! {
            $crate::sso::AuthEventSinkRegistration {
                factory: $factory,
            }
        }
    };
}
