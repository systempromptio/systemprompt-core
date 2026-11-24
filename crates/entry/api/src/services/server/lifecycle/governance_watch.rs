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

use anyhow::Result;
use systemprompt_database::{DbPool, PgConfigVersion};
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    BackgroundTasks, CONFIG_WATCH_POLL, CacheDomain, CacheEpochs, ConfigWatch, WatchSignals,
};

pub(in crate::services::server) fn start_governance_watch(ctx: &AppContext) -> Result<()> {
    let state = ctx.kernel().state();
    let signals = WatchSignals {
        domain: CacheDomain::Governance,
        epochs: state.get::<CacheEpochs>()?.clone(),
        version: Arc::new(PgConfigVersion::new(state.get::<DbPool>()?)),
        poll: CONFIG_WATCH_POLL,
    };
    let kernel = Arc::clone(ctx.kernel());
    state.get::<ConfigWatch<GovernanceEngine>>()?.spawn(
        state.get::<BackgroundTasks>()?,
        signals,
        move || {
            let kernel = Arc::clone(&kernel);
            async move { systemprompt_runtime::reload_governance(&kernel).await }
        },
    );
    tracing::info!("Governance engine follows the governance config plane");
    Ok(())
}
