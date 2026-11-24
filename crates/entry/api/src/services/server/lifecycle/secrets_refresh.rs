//! Node-scoped secrets reload.
//!
//! The secrets store is per process, so a reload has to run in every process
//! that holds one — every replica and every role, `gateway` included, since
//! that is where provider keys are used. This task is started on all roles and
//! takes no cluster lock: each replica re-reads its own source on the profile's
//! interval (`secrets.refresh_secs` for a `file` source,
//! `secrets.external.refresh_secs` for an `external` one). With neither set it
//! is not started. A reload that would change a pinned key is refused and the
//! current generation stays, exactly as for a manual reload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_loader::BootContext;
use systemprompt_runtime::AppContext;
use systemprompt_traits::BackgroundTasks;

pub(in crate::services::server) fn start_secrets_refresh(ctx: &AppContext) -> Result<()> {
    let state = ctx.kernel().state();
    let boot = state.get::<BootContext>()?;
    let Some(interval) = boot.secrets().refresh_interval() else {
        return Ok(());
    };
    tracing::info!(
        interval_secs = interval.as_secs(),
        "secrets refresh scheduled on this replica"
    );
    let secrets = Arc::clone(boot.secrets());
    state.get::<BackgroundTasks>()?.spawn_cancellable(
        "secrets_refresh",
        move |cancel| async move {
            let mut tick = tokio::time::interval(interval);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            tick.tick().await;
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {},
                }
                match secrets.reload().await {
                    Ok(outcome) if outcome.swapped => tracing::info!(
                        generation = outcome.generation,
                        changed = ?outcome.changed_keys,
                        "secrets refresh applied a new generation"
                    ),
                    Ok(_) => tracing::debug!("secrets refresh found no change"),
                    Err(error) => tracing::warn!(
                        error = %error,
                        generation = secrets.generation(),
                        "secrets refresh failed; keeping the current generation"
                    ),
                }
            }
        },
    );
    Ok(())
}
