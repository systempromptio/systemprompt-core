//! [`AppContext`] — the application-wide runtime container.
//!
//! Holds shared handles (config, database pool, route classifier, etc.)
//! cloned cheaply via [`Arc`], the admitted composition and the [`Kernel`]
//! resolved over it. Constructed via [`crate::AppContextBuilder`] or
//! [`AppContext::new`]; both end in [`AppContext::from_parts`], which takes
//! the resolved kernel, so the context's extension state is the kernel's
//! state and no context exists without it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_analytics::{AnalyticsService, GeoIpReader};
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_events::{EventBridgeHandle, EventRouter};
use systemprompt_extension::Admitted;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_loader::BootContext;
use systemprompt_manifest::Config;
use systemprompt_models::ContentConfigRaw;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::FileStorage;

mod accessors;
mod context_loaders;
mod debug_impls;
mod planes;
mod repositories;
mod services;
mod shutdown;

pub use planes::{ConfigPlane, ContextPlanes, DataPlane, Plugins, Subsystems};
pub use shutdown::ShutdownRequest;

use crate::builder::AppContextBuilder;
use crate::error::RuntimeResult;
use crate::kernel::Kernel;

/// Application-wide runtime container shared across the HTTP server, the
/// scheduler, and CLI commands.
///
/// Handles are grouped into four cohesive planes ([`DataPlane`],
/// [`ConfigPlane`], [`Plugins`], [`Subsystems`]); each field is an [`Arc`] (or
/// an `Arc`-internal handle such as [`DbPool`]), so `clone` is a
/// reference-count bump rather than a deep copy. Construct it via
/// [`AppContext::builder`] or [`AppContext::new`]; [`AppContext::from_parts`]
/// bypasses the bootstrap for tests and embedders that assemble the planes
/// and resolve the kernel themselves.
#[derive(Clone)]
pub struct AppContext {
    pub(crate) data: DataPlane,
    pub(crate) cfg: ConfigPlane,
    pub(crate) plugins: Plugins,
    pub(crate) subsystems: Subsystems,
    pub(crate) kernel: Arc<Kernel>,
    pub(crate) admitted: Arc<Admitted>,
    pub(crate) analytics: Arc<AnalyticsService>,
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
        kernel: Arc<Kernel>,
        admitted: Arc<Admitted>,
    ) -> RuntimeResult<Self> {
        let ContextPlanes {
            data,
            cfg,
            plugins,
            subsystems,
        } = planes;
        let analytics = kernel.resolved().get::<Arc<AnalyticsService>>()?;
        Ok(Self {
            data,
            cfg,
            plugins,
            subsystems,
            kernel,
            admitted,
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

    pub fn load_geoip_database(
        config: &Config,
        show_warnings: bool,
    ) -> Result<Option<GeoIpReader>, crate::error::RuntimeError> {
        context_loaders::load_geoip_database(config, show_warnings)
    }

    pub fn load_content_config(
        config: &Config,
        app_paths: &AppPaths,
    ) -> Option<Arc<ContentConfigRaw>> {
        context_loaders::load_content_config(config, app_paths)
    }

    pub fn config(&self) -> &Config {
        &self.cfg.config
    }

    pub fn content_config(&self) -> Option<&ContentConfigRaw> {
        self.cfg.content_config.as_ref().map(AsRef::as_ref)
    }

    pub const fn db_pool(&self) -> &DbPool {
        &self.data.database
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.cfg.config.host, self.cfg.config.port)
    }

    pub const fn geoip_reader(&self) -> Option<&GeoIpReader> {
        self.subsystems.geoip_reader.as_ref()
    }

    pub const fn file_storage(&self) -> &Arc<dyn FileStorage> {
        &self.subsystems.file_storage
    }

    #[must_use]
    pub fn session_store(&self) -> Option<systemprompt_traits::DynSessionStore> {
        self.kernel
            .state()
            .find::<systemprompt_traits::DynSessionStore>()
            .cloned()
    }


    pub fn app_paths(&self) -> &AppPaths {
        &self.cfg.app_paths
    }

    pub const fn app_paths_arc(&self) -> &Arc<AppPaths> {
        &self.cfg.app_paths
    }

    pub const fn event_bridge(&self) -> &Arc<EventBridgeHandle> {
        &self.subsystems.event_bridge
    }

    pub const fn event_router(&self) -> &EventRouter {
        &self.subsystems.event_router
    }

    pub const fn subject_providers(&self) -> &systemprompt_governance::authz::SubjectProviderSet {
        &self.plugins.subject_providers
    }

    pub const fn authz_hook(&self) -> &SharedAuthzHook {
        &self.subsystems.authz_hook
    }

    #[must_use]
    pub const fn signing_keys(&self) -> &SigningKeys {
        &self.subsystems.signing_keys
    }

    #[must_use]
    pub fn schema_state(&self) -> &crate::SchemaState {
        &self.subsystems.schema_install
    }
}
