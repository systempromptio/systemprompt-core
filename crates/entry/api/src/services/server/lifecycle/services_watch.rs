//! Keeps the services config every request reads in step with the served
//! services tree.
//!
//! The config comes from the services `ConfigWatch` on the `AppContext`. This
//! task re-reads the tree through the active root (the bundle cache's
//! `current` link a refresh swaps) whenever a refresh advances the `services`
//! epoch, and on a `config_version` move as the fallback, so a recomposed
//! tree is served without a restart. A tree that fails to load keeps the last
//! good config and logs the error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::PgConfigVersion;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{CONFIG_WATCH_POLL, CacheDomain, WatchSignals};

pub(in crate::services::server) fn start_services_watch(ctx: &AppContext) {
    let signals = WatchSignals {
        domain: CacheDomain::Services,
        epochs: ctx.cache_epochs().clone(),
        version: Arc::new(PgConfigVersion::new(ctx.db_pool())),
        poll: CONFIG_WATCH_POLL,
    };
    ctx.services_watch()
        .spawn(ctx.background_tasks(), signals, || async {
            ConfigLoader::reload()
        });
    tracing::info!("Services config follows the served services tree");
}
