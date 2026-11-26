//! The gateway repositories a mount builds.
//!
//! [`gateway_repositories`] composes them from a resolution and the host
//! handles: the accounting journal and at-rest cipher come from
//! [`GatewayAtRest`], so nothing reads the secret store; the session usage
//! counters are required, and the artifact ingest and brain sink are read
//! optionally (disabling their member leaves the gateway without them). The
//! `gateway-http` surface calls it with its route-scoped state; kernel code
//! (accounting recovery, the in-process client) with the kernel's own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::DynBrainSink;
use systemprompt_extension::mcp::artifacts::DynToolResultIngest;
use systemprompt_extension::prelude::Resolved;
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_traits::{CacheEpochView, DynContextMaterializer, DynSessionUsageCounters};

use systemprompt_extension::router::GatewaySurfaceError;

use super::GatewayHostHandles;
use crate::audit::session::SessionCapture;
use crate::{GatewayAtRest, GatewayHandles, GatewayRepositories};

pub fn gateway_repositories(
    r: &Resolved<'_>,
    host: &GatewayHostHandles,
    contexts: Option<DynContextMaterializer>,
    db: &DbPool,
) -> Result<GatewayRepositories, GatewaySurfaceError> {
    let at_rest = r.get::<Arc<GatewayAtRest>>()?;
    let subject_providers = r.get::<Arc<SubjectProviderSet>>()?;
    let repos = GatewayRepositories::new(
        db,
        at_rest
            .journal()
            .map_err(|source| GatewaySurfaceError::Journal(Box::new(source)))?,
        contexts,
        GatewayHandles {
            background: r.tasks().clone(),
            cache_epochs: r.get::<CacheEpochView>()?,
            logging: host.logging.clone(),
            services: host.services.view(),
            subject_providers: (*subject_providers).clone(),
            at_rest: at_rest.cipher(),
            unapplied_plane: host.unapplied_plane,
        },
    )
    .with_payload_cap(host.payload_cap_bytes)
    .with_quota_sharding(host.quota)
    .with_session_usage(r.get::<DynSessionUsageCounters>()?)
    .with_brain_recall(host.brain.recall.clone());
    let repos = match r.optional::<DynToolResultIngest>() {
        Some(ingest) => repos.with_artifact_ingest(ingest),
        None => repos,
    };
    let brain_sink = r.optional::<DynBrainSink>().filter(|_| host.brain.enabled);
    Ok(match brain_sink {
        Some(sink) => repos.with_brain_session(SessionCapture::new(
            sink,
            host.brain.capture.clone(),
            Arc::clone(db),
        )),
        None => repos,
    })
}
