//! Writing each host's settings for a verified manifest, one emitter at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_models::bridge::host::HostKind;

use super::{ApplyError, HostFailure, HostWarning};
use crate::context::BridgeContext;
use crate::host_sync::{self, HostSync, HostSyncCtx};

pub(super) struct HostsPhase {
    pub warnings: Vec<HostWarning>,
    pub failures: Vec<HostFailure>,
    pub skipped_absent: BTreeSet<HostKind>,
}

pub(super) async fn emit_hosts(
    emitters: &[&'static dyn HostSync],
    ctx: &HostSyncCtx<'_>,
    bridge: &BridgeContext,
) -> HostsPhase {
    for (skill, host) in crate::gateway::manifest::unknown_skill_hosts(ctx.manifest) {
        tracing::warn!(
            target: "bridge::sync::host",
            skill,
            host,
            "skill targets a host id this bridge does not know; no host receives it under that id"
        );
    }
    let mut phase = HostsPhase {
        warnings: Vec::new(),
        failures: Vec::new(),
        skipped_absent: BTreeSet::new(),
    };
    for (index, emitter) in emitters.iter().enumerate() {
        let host_id = emitter.host_id();
        bridge
            .sync_progress
            .report(&crate::progress::SyncProgress::new(
                "hosts",
                host_id.as_str().to_owned(),
                index + 1,
                emitters.len(),
            ));
        let enabled = host_sync::enabled_for(*emitter, ctx.manifest);
        if enabled && app_absent(*emitter, &bridge.start_menu) {
            tracing::info!(
                host = host_id.as_str(),
                "host app not installed; settings not written"
            );
            phase.skipped_absent.insert(host_id);
            continue;
        }
        match emit_one(*emitter, ctx, enabled).await {
            Ok(warnings) => phase.warnings.extend(warnings),
            Err(failure) => phase.failures.push(failure),
        }
    }
    phase
}

async fn emit_one(
    emitter: &'static dyn HostSync,
    ctx: &HostSyncCtx<'_>,
    enabled: bool,
) -> Result<Vec<HostWarning>, HostFailure> {
    let outcome = if enabled {
        emitter.apply(ctx).await
    } else {
        emitter.clear(ctx).map(|()| host_sync::HostSyncReport::ok())
    };
    match outcome {
        Ok(emitted) => {
            host_sync::log_outcome(emitter, enabled, Ok(()));
            Ok(emitted.warnings)
        },
        Err(e) => {
            host_sync::log_outcome(emitter, enabled, Err(&e));
            Err(HostFailure {
                host_id: emitter.host_id(),
                emitter: emitter.emitter_id().to_owned(),
                error: format!("{e:#}"),
                needs_elevation: matches!(e, ApplyError::ElevationRequired { .. }),
            })
        },
    }
}

// Why: an emitter serving both packagings of a product writes for whichever is
// present; only when every packaging it serves is conclusively absent is the
// tree left unwritten.
fn app_absent(emitter: &dyn HostSync, start_menu: &crate::probe_cache::StartMenuCache) -> bool {
    let served: Vec<&'static dyn crate::integration::HostApp> = crate::integration::host_apps()
        .iter()
        .copied()
        .filter(|host| emitter.serves(host.id()))
        .collect();
    !served.is_empty()
        && served.iter().all(|host| {
            host.app_installed(start_menu) == crate::integration::AppInstallState::NotInstalled
        })
}
