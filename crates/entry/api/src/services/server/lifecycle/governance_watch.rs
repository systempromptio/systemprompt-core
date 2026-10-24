//! Keeps the governance engine in step with the `governance` config plane.
//!
//! The engine every request reads comes from the `ConfigWatch` on the
//! `AppContext`. This task rebuilds it from `governance_config` whenever the
//! plane's `config:governance` notification advances the governance epoch,
//! and on a `config_version` poll as the fallback for a lost notification, so
//! an apply or a console edit on any node changes enforcement here without a
//! restart. A rebuild that fails keeps the last good engine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::PgConfigVersion;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{CONFIG_WATCH_POLL, CacheDomain, WatchSignals};

pub(in crate::services::server) fn start_governance_watch(ctx: &AppContext) {
    let signals = WatchSignals {
        domain: CacheDomain::Governance,
        epochs: ctx.cache_epochs().clone(),
        version: Arc::new(PgConfigVersion::new(ctx.db_pool())),
        poll: CONFIG_WATCH_POLL,
    };
    let reload_ctx = ctx.clone();
    ctx.governance_watch()
        .spawn(ctx.background_tasks(), signals, move || {
            let ctx = reload_ctx.clone();
            async move { ctx.load_governance().await }
        });
    tracing::info!("Governance engine follows the governance config plane");
}
