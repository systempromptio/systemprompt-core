//! Periodic heartbeat on this replica's `services` rows, and the reaping of
//! rows whose replica stopped heartbeating.
//!
//! Every replica heartbeats every [`HEARTBEAT_INTERVAL`], recording its
//! `server.role` presence and refreshing its advertised endpoint when
//! `server.advertise_url` is set; every
//! [`GC_EVERY_BEATS`]th beat it also deletes rows whose heartbeat is older
//! than [`INSTANCE_DEAD_AFTER_SECS`], the same window
//! [`systemprompt_traits::InstanceRegistry::is_alive`] judges by. The delete is
//! idempotent, so replicas reaping concurrently is harmless, and a deployment
//! with any live replica keeps its registry clean without a scheduled job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{INSTANCE_DEAD_AFTER_SECS, ServiceRepository};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_runtime::AppContext;

pub(in crate::services::server) const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

const GC_EVERY_BEATS: u32 = 4;

pub(in crate::services::server) fn start_registry_heartbeat(ctx: &AppContext) {
    let repository = Arc::clone(ctx.service_repository());
    let role = ctx.profile().server.role;
    let advertise_url = ctx
        .config()
        .advertise_url
        .clone()
        .filter(|url| !url.is_empty());
    ctx.background_tasks()
        .spawn_cancellable("registry_heartbeat", |cancel| async move {
            let mut tick = tokio::time::interval(HEARTBEAT_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut beats: u32 = 0;
            loop {
                tokio::select! {
                    () = cancel.cancelled() => break,
                    _ = tick.tick() => {},
                }
                beat(&repository, role, advertise_url.as_deref(), &mut beats).await;
            }
        });
}

async fn beat(
    repository: &ServiceRepository,
    role: NodeRole,
    advertise_url: Option<&str>,
    beats: &mut u32,
) {
    if let Err(error) = repository.touch_heartbeat().await {
        tracing::warn!(
            instance_id = %repository.instance_id(),
            error = %error,
            "service registry heartbeat failed"
        );
    }
    if let Err(error) = repository.touch_presence(role).await {
        tracing::warn!(
            instance_id = %repository.instance_id(),
            role = %role,
            error = %error,
            "instance presence heartbeat failed"
        );
    }
    if let Some(url) = advertise_url
        && let Err(error) = repository.advertise_endpoint(url).await
    {
        tracing::warn!(
            instance_id = %repository.instance_id(),
            error = %error,
            "advertised endpoint heartbeat failed"
        );
    }
    *beats = beats.wrapping_add(1);
    if beats.is_multiple_of(GC_EVERY_BEATS) {
        reap(repository).await;
    }
}

async fn reap(repository: &ServiceRepository) {
    match repository
        .delete_dead_instances(INSTANCE_DEAD_AFTER_SECS)
        .await
    {
        Ok(0) => {},
        Ok(reaped) => tracing::info!(reaped, "service registry reaped dead instances"),
        Err(error) => {
            tracing::warn!(error = %error, "service registry reap failed");
        },
    }
    if let Err(error) = repository
        .delete_stale_endpoints(INSTANCE_DEAD_AFTER_SECS)
        .await
    {
        tracing::warn!(error = %error, "advertised endpoint reap failed");
    }
    if let Err(error) = repository
        .delete_stale_presence(INSTANCE_DEAD_AFTER_SECS)
        .await
    {
        tracing::warn!(error = %error, "instance presence reap failed");
    }
}
