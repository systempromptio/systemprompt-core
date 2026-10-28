//! Compile-time registration of SSO providers and sign-in audit sinks.
//!
//! A downstream crate submits a factory with [`register_sso_provider!`] or
//! [`register_auth_event_sink!`]; [`super::SsoDriver::discover`] runs every
//! factory once at a composition root. A provider factory returning
//! `Ok(None)` means "configured off", so a provider can stay linked but
//! disabled; `Err` means "configured on but unusable" and fails discovery, so
//! a broken sign-in configuration stops the boot instead of vanishing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::SsoProviderId;
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

pub type SsoProviderFactory =
    fn(&SsoHookContext) -> Result<Option<Arc<dyn SsoProvider>>, SsoProviderError>;

/// One inventory submission per
/// [`register_sso_provider!`](crate::register_sso_provider) call.
#[derive(Debug, Clone, Copy)]
pub struct SsoProviderRegistration {
    pub factory: SsoProviderFactory,
}

/// Why SSO provider discovery refused to build the provider set.
#[derive(Debug, thiserror::Error)]
pub enum SsoProviderError {
    #[error("SSO provider `{provider_id}` is enabled but its configuration is unusable: {source}")]
    Config {
        provider_id: SsoProviderId,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("SSO provider id `{provider_id}` is registered more than once")]
    Duplicate { provider_id: SsoProviderId },
}

impl SsoProviderError {
    pub fn config(
        provider_id: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            provider_id: SsoProviderId::new(provider_id),
            source: Box::new(source),
        }
    }
}

inventory::collect!(SsoProviderRegistration);

/// One inventory submission per
/// [`register_auth_event_sink!`](crate::register_auth_event_sink) call.
#[derive(Debug, Clone, Copy)]
pub struct AuthEventSinkRegistration {
    pub factory: fn(&SsoHookContext) -> Arc<dyn AuthEventSink>,
}

inventory::collect!(AuthEventSinkRegistration);

pub(super) fn discover_providers(
    ctx: &SsoHookContext,
) -> Result<Vec<Arc<dyn SsoProvider>>, SsoProviderError> {
    let mut providers: Vec<Arc<dyn SsoProvider>> = Vec::new();
    for registration in inventory::iter::<SsoProviderRegistration>() {
        let Some(provider) = (registration.factory)(ctx)? else {
            continue;
        };
        if providers.iter().any(|p| p.id() == provider.id()) {
            return Err(SsoProviderError::Duplicate {
                provider_id: SsoProviderId::new(provider.id()),
            });
        }
        providers.push(provider);
    }
    Ok(providers)
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
