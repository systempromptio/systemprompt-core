//! [`AppContext`] — the handle a host holds on the resolved kernel.
//!
//! An [`AppContext`] is the admitted composition and the [`Kernel`] resolved
//! over it, and nothing else. Every host value — the runtime `Config`, the
//! database pool, the boot context, the platform repositories, the
//! governance and services watches, the event bridge, the shutdown request —
//! is a typed seed in the kernel state, read with
//! `ctx.kernel().state().get::<T>()` by the host code that needs it. The
//! kernel seeds most of them before it resolves ([`crate::kernel_state`]);
//! [`AppContext::from_parts`] seals the rest ([`ContextPlanes`]) into the
//! state after the members resolved, every one sealed and every one's type
//! reserved before resolution, so no member provides, reads or is handed one.
//! A kernel that lacks a host seed refuses the context with the typed
//! [`RuntimeError::Missing`](crate::RuntimeError::Missing) naming it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::Admitted;
use systemprompt_loader::BootContext;
use systemprompt_manifest::Config;
use systemprompt_models::RouteClassifier;
use systemprompt_traits::AnalyticsProvider;

mod context_loaders;
mod debug_impls;
mod host;
mod planes;
mod shutdown;

pub(crate) use context_loaders::load_content_config;
pub(crate) use host::reserve_context_seeds;
pub use host::{config_plane_set, reload_governance};
pub use planes::{ConfigPlane, ContextPlanes, DataPlane, Plugins, Subsystems};
pub use shutdown::ShutdownRequest;

use crate::builder::AppContextBuilder;
use crate::error::RuntimeResult;
use crate::kernel::Kernel;

/// The admitted composition and the kernel resolved over it, shared across
/// the HTTP server, the scheduler and CLI commands.
///
/// `clone` is two reference-count bumps. Construct it via
/// [`AppContext::builder`] or [`AppContext::new`];
/// [`AppContext::from_parts`] takes a resolved kernel and the
/// [`ContextPlanes`] for tests and embedders that assemble them themselves.
#[derive(Clone)]
pub struct AppContext {
    kernel: Arc<Kernel>,
    admitted: Arc<Admitted>,
    pub(crate) config: Arc<Config>,
    pub(crate) database: systemprompt_database::DbPool,
    pub(crate) route_classifier: Arc<RouteClassifier>,
    pub(crate) analytics: Arc<dyn AnalyticsProvider>,
}

impl AppContext {
    pub async fn new(boot: BootContext, admitted: Arc<Admitted>) -> RuntimeResult<Self> {
        Self::builder(boot, admitted).build().await
    }

    #[must_use]
    pub fn builder(boot: BootContext, admitted: Arc<Admitted>) -> AppContextBuilder {
        AppContextBuilder::new(boot, admitted)
    }

    pub fn from_parts(
        planes: ContextPlanes,
        mut kernel: Kernel,
        admitted: Arc<Admitted>,
    ) -> RuntimeResult<Self> {
        host::require_kernel_seeds(kernel.state())?;
        let route_classifier = Arc::clone(&planes.cfg.route_classifier);
        host::seal_context_seeds(&mut kernel, planes);
        let state = kernel.state();
        let config = Arc::clone(state.get::<Arc<Config>>()?);
        let database = Arc::clone(state.get::<systemprompt_database::DbPool>()?);
        let analytics = kernel.resolved().get::<Arc<dyn AnalyticsProvider>>()?;
        Ok(Self {
            kernel: Arc::new(kernel),
            admitted,
            config,
            database,
            route_classifier,
            analytics,
        })
    }

    #[must_use]
    pub const fn kernel(&self) -> &Arc<Kernel> {
        &self.kernel
    }

    #[must_use]
    pub const fn admitted(&self) -> &Arc<Admitted> {
        &self.admitted
    }
}
