//! The device-credential consumer surface the bridge reports installation
//! evidence through, built by [`ManagedHttpSurface`].
//!
//! Only the routes a bridge calls are served: device enrollment, the host
//! installation plan (bundle), installation receipts and session bindings.
//! Every request authenticates with an enrolled device's `sp_device_`
//! credential; the consumer identity is derived from it, never accepted from
//! JSON. Administration of managed resources happens in-process through the
//! marketplace repository, not over HTTP.
//!
//! The route state ([`ManagedConsumerState`]) holds the managed repository
//! and the services view the `managed_resources` member and the kernel
//! provide, the API's external URL the origin guard compares against, and the
//! host's `ConsumerCatalog` read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod consumer;
pub mod contract;
pub mod origin;

use std::sync::Arc;

use systemprompt_extension::MissingDependency;
use systemprompt_manifest::services::ServicesView;
use systemprompt_marketplace::managed::ManagedRepository;
use systemprompt_extension::router::{
    DynConsumerCatalog, ManagedRouteState, ManagedRoutes, ManagedSurface,
};

use super::MarketplaceHttpSettings;

/// Router-scoped state of the managed consumer routes.
#[derive(Clone)]
pub struct ManagedConsumerState {
    pub(crate) repository: Arc<ManagedRepository>,
    pub(crate) services: ServicesView,
    pub(crate) api_external_url: Arc<str>,
    pub(crate) catalog: DynConsumerCatalog,
}

impl std::fmt::Debug for ManagedConsumerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedConsumerState")
            .field("api_external_url", &self.api_external_url)
            .finish_non_exhaustive()
    }
}

/// The `marketplace-http` member's managed consumer surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct ManagedHttpSurface;

impl ManagedConsumerState {
    #[must_use]
    pub const fn new(
        repository: Arc<ManagedRepository>,
        services: ServicesView,
        api_external_url: Arc<str>,
        catalog: DynConsumerCatalog,
    ) -> Self {
        Self {
            repository,
            services,
            api_external_url,
            catalog,
        }
    }
}

impl ManagedSurface for ManagedHttpSurface {
    fn build(&self, state: ManagedRouteState<'_>) -> Result<ManagedRoutes, MissingDependency> {
        let settings = state.resolved.get::<MarketplaceHttpSettings>()?;
        Ok(routes(ManagedConsumerState::new(
            state.resolved.get()?,
            state.resolved.get()?,
            settings.api_external_url,
            state.catalog,
        )))
    }
}

pub fn routes(state: ManagedConsumerState) -> ManagedRoutes {
    let router = consumer::router()
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state.api_external_url),
            origin::protect,
        ))
        .with_state(state);
    ManagedRoutes {
        consumer: router,
        normalize: Box::new(|protected| {
            protected.layer(axum::middleware::from_fn(contract::normalize))
        }),
    }
}
