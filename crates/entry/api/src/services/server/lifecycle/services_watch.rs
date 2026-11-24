//! Keeps the services config every request reads in step with the served
//! services tree.
//!
//! The config comes from the services `ConfigWatch` on the `AppContext`. This
//! task re-reads the tree through the active root (the bundle cache's
//! `current` link a refresh swaps) whenever a refresh advances the `services`
//! epoch, and on a moved version as the fallback, so a recomposed tree is
//! served without a restart. The version it polls folds the database
//! `config_version` together with a fingerprint of the files under the active
//! root (`TreeAndDbVersion`), so an edit on disk — a mounted `ConfigMap` a
//! `GitOps` sync updates — reloads within one poll interval too. A tree that
//! fails to load keeps the last good config and logs the error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use systemprompt_database::{DbPool, PgConfigVersion};
use systemprompt_loader::{BootContext, ServicesState, tree_fingerprint};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    BackgroundTasks, CONFIG_WATCH_POLL, CacheDomain, CacheEpochs, ConfigVersionSource,
    RepositoryError, WatchSignals,
};

struct TreeAndDbVersion {
    db: Arc<dyn ConfigVersionSource>,
    boot: BootContext,
}

impl std::fmt::Debug for TreeAndDbVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeAndDbVersion").finish_non_exhaustive()
    }
}

impl TreeAndDbVersion {
    fn new(db: Arc<dyn ConfigVersionSource>, boot: BootContext) -> Self {
        Self { db, boot }
    }
}

#[async_trait]
impl ConfigVersionSource for TreeAndDbVersion {
    async fn current_version(&self) -> Result<i64, RepositoryError> {
        let db = self.db.current_version().await?;
        let root = self.boot.require_services_root().path;
        let tree = match tokio::task::spawn_blocking(move || tree_fingerprint(&root)).await {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::warn!(%error, "services tree fingerprint failed; forcing a reload");
                0
            },
        };
        let mut hasher = DefaultHasher::new();
        db.hash(&mut hasher);
        tree.hash(&mut hasher);
        Ok(i64::from_ne_bytes(hasher.finish().to_ne_bytes()))
    }
}

pub(in crate::services::server) fn start_services_watch(ctx: &AppContext) -> Result<()> {
    let state = ctx.kernel().state();
    let boot = state.get::<BootContext>()?.clone();
    let signals = WatchSignals {
        domain: CacheDomain::Services,
        epochs: state.get::<CacheEpochs>()?.clone(),
        version: Arc::new(TreeAndDbVersion::new(
            Arc::new(PgConfigVersion::new(state.get::<DbPool>()?)),
            boot.clone(),
        )),
        poll: CONFIG_WATCH_POLL,
    };
    state.get::<ServicesState>()?.watch().spawn(
        state.get::<BackgroundTasks>()?,
        signals,
        move || {
            let boot = boot.clone();
            async move { boot.load_services_config() }
        },
    );
    tracing::info!("Services config follows the served services tree and its files on disk");
    Ok(())
}
