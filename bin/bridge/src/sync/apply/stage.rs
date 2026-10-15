//! Staging every changed plugin concurrently, before any is promoted.
//!
//! A changed plugin arrives as one content-addressed archive; a gateway that
//! predates the archive route (404) is served file by file instead. Downloads
//! run a few plugins at a time, while promotion stays sequential in manifest
//! order, so cancellation still lands only between plugins.
//!
//! Each job owns its inputs (a cloned client, the bearer, the entry): a borrow
//! held across the buffered await trips rustc's higher-ranked `Send` check
//! once the sync future is spawned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;

use futures_util::StreamExt;

use super::ApplyError;
use super::archive::extract_verified;
use super::fetch::fetch_plugin_into_staging;
use super::plugin::PluginSyncCtx;
use crate::gateway::manifest::PluginEntry;
use crate::gateway::{ArchiveResponse, GatewayClient};
use crate::ids::BearerToken;
use crate::progress::SyncProgress;

const PLUGIN_STAGE_CONCURRENCY: usize = 4;

pub(super) enum Staged {
    Complete,
    Cancelled,
}

struct StageJob {
    client: GatewayClient,
    bearer: BearerToken,
    plugin: PluginEntry,
    stage: PathBuf,
    installed: PathBuf,
}

/// Stages `changed` under the staging root; `already` plugins are unchanged
/// and count as done for progress out of `total`.
pub(super) async fn stage_changed(
    ctx: &PluginSyncCtx<'_>,
    changed: &[&PluginEntry],
    already: usize,
    total: usize,
) -> Result<Staged, ApplyError> {
    let jobs: Vec<_> = changed
        .iter()
        .map(|plugin| {
            stage_one(StageJob {
                client: ctx.client.clone(),
                bearer: ctx.bearer.clone(),
                plugin: (*plugin).clone(),
                stage: ctx.staging_root.join(plugin.id.as_str()),
                installed: ctx.root.join(plugin.id.as_str()),
            })
        })
        .collect();
    let mut jobs = futures_util::stream::iter(jobs).buffer_unordered(PLUGIN_STAGE_CONCURRENCY);
    let cancel = ctx.cancel.clone();
    let mut done = already;
    loop {
        let next = tokio::select! {
            () = cancel.cancelled() => return Ok(Staged::Cancelled),
            next = jobs.next() => next,
        };
        let Some(staged) = next else {
            return Ok(Staged::Complete);
        };
        let plugin_id = staged?;
        done += 1;
        ctx.progress
            .report(&SyncProgress::new("plugins", plugin_id, done, total));
    }
}

async fn stage_one(job: StageJob) -> Result<String, ApplyError> {
    let StageJob {
        client,
        bearer,
        plugin,
        stage,
        installed,
    } = job;
    fs::create_dir_all(&stage).map_err(|e| ApplyError::Io {
        context: format!("create stage {}", stage.display()),
        source: e,
    })?;
    match client
        .fetch_plugin_archive(&bearer, &plugin.id, &plugin.sha256)
        .await?
    {
        ArchiveResponse::Archive(bytes) => {
            let plugin_id = plugin.id.clone();
            tokio::task::spawn_blocking(move || extract_verified(&bytes, &plugin, &stage))
                .await
                .map_err(|error| ApplyError::Io {
                    context: format!("unpack the archive for {plugin_id}"),
                    source: std::io::Error::other(error),
                })??;
            Ok(plugin_id.to_string())
        },
        ArchiveResponse::Unsupported => {
            tracing::debug!(
                target: "bridge::sync::fetch",
                plugin_id = %plugin.id,
                "gateway serves no plugin archives; fetching file by file"
            );
            fetch_plugin_into_staging(&client, &bearer, &plugin, &stage, &installed).await?;
            Ok(plugin.id.to_string())
        },
        ArchiveResponse::Stale => Err(ApplyError::StaleManifest(plugin.id)),
    }
}
