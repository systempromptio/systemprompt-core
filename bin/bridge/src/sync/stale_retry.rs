//! One retry for a sync whose manifest went stale mid-run.
//!
//! Plugin archives are addressed by the digest the manifest signed; a gateway
//! that has moved on answers 409 for the old digest. The run is then repeated
//! once against a freshly built manifest, never more: a second stale answer is
//! a gateway fault, and is reported as the failure it is.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{SyncError, SyncOptions, SyncSummary, apply, run_attempt};
use crate::gateway::Freshness;

pub async fn run_once(
    bridge: &crate::context::BridgeContext,
    options: &SyncOptions,
) -> Result<SyncSummary, SyncError> {
    match Box::pin(run_attempt(bridge, options)).await {
        Err(SyncError::ApplyFailed(error))
            if matches!(*error, apply::ApplyError::StaleManifest(_)) =>
        {
            tracing::info!(
                target: "bridge::sync",
                %error,
                "manifest went stale during the sync; refetching it once"
            );
            let fresh = SyncOptions {
                allow_unsigned: options.allow_unsigned,
                force_replay: options.force_replay,
                allow_tofu: options.allow_tofu,
                freshness: Freshness::Fresh,
                cancel: options.cancel.clone(),
            };
            Box::pin(run_attempt(bridge, &fresh)).await
        },
        other => other,
    }
}
