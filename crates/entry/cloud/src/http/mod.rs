//! The services-tree sync routes: the file manifest and download handlers
//! `systemprompt cloud backup` pulls agent, skill, content and config
//! definitions through.
//!
//! [`CloudSyncSurface`] is the [`SyncSurface`] the `cloud` member provides;
//! the API mounts what it builds under `/api/v1/sync` behind its own
//! declaration (service callers only, a user request context). The route
//! state is the services root the kernel's `AppPaths` names, a plain path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::router::SyncSurface;
use systemprompt_extension::{MissingDependency, Resolved};

mod archive;
mod files;
mod types;

/// Router-scoped state of the sync routes: the services root.
#[derive(Debug, Clone)]
pub struct SyncState {
    services_root: PathBuf,
}

/// The `cloud` member's services-tree sync surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct CloudSyncSurface;

impl SyncSurface for CloudSyncSurface {
    fn build(&self, resolved: &Resolved<'_>) -> Result<Router, MissingDependency> {
        let paths = resolved.get::<Arc<AppPaths>>()?;
        Ok(router(SyncState {
            services_root: paths.system().services().to_path_buf(),
        }))
    }
}

pub fn router(state: SyncState) -> Router {
    Router::new()
        .route("/files", get(files::download))
        .route("/files/manifest", get(files::manifest))
        .with_state(state)
}

impl SyncState {
    #[must_use]
    pub const fn new(services_root: PathBuf) -> Self {
        Self { services_root }
    }
}
