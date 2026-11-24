//! The inputs an SSO provider, sign-in audit sink or role resolver factory
//! reads while it is built.
//!
//! Members contribute those factories to
//! [`SsoSlot`](crate::contract::SsoSlot), whose build runs each once. A
//! factory reads its inputs with [`SsoHookContext::from_resolved`]. A
//! provider factory returning `Ok(None)` means "configured off", so a provider
//! can stay linked but disabled; an [`SsoProviderError`] means "configured on
//! but unusable" and fails the slot build, so a broken sign-in configuration
//! stops the boot instead of vanishing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::{MissingDependency, Resolved};
use systemprompt_identifiers::SsoProviderId;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_security::http::EdgeConfigView;
use systemprompt_traits::SecretStore;

/// What a factory may read while building its provider or sink.
#[derive(Clone)]
pub struct SsoHookContext {
    pub db: DbPool,
    pub edge: EdgeConfigView,
    pub services: ServicesView,
    pub services_root: ActiveServicesRoot,
    pub secrets: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for SsoHookContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoHookContext")
            .field("api_external_url", &self.edge.api_external_url)
            .finish_non_exhaustive()
    }
}

impl SsoHookContext {
    pub fn from_resolved(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self {
            db: r.get()?,
            edge: EdgeConfigView::of(r.get::<Arc<Config>>()?.as_ref()),
            services: r.get()?,
            services_root: r.get()?,
            secrets: r.get()?,
        })
    }
}

/// Why an SSO provider factory refused to build its provider.
#[derive(Debug, thiserror::Error)]
pub enum SsoProviderError {
    #[error("SSO provider `{provider_id}` is enabled but its configuration is unusable: {source}")]
    Config {
        provider_id: SsoProviderId,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
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
