//! Installation-evidence capture after the host emitters have applied: every
//! enabled, non-failed host at once, each within its own budget, reported as
//! a typed outcome rather than a deadline error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ApplyReport, HostFailure};
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::{HostSync, HostSyncCtx, HostWarningKind, HostWarnings};
use crate::ids::HostId;

pub(super) async fn capture(
    emitters: &[&'static dyn HostSync],
    manifest: &SignedManifest,
    ctx: &HostSyncCtx<'_>,
    warnings: &HostWarnings,
    report: &mut ApplyReport,
) {
    let hosts: Vec<&str> = emitters
        .iter()
        .map(|emitter| emitter.host_id())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|host_id| {
            manifest.enabled_hosts.iter().any(|host| host == host_id)
                && !report
                    .host_failures
                    .iter()
                    .any(|failure| failure.host_id.as_str() == *host_id)
        })
        .collect();
    // Why: each host's evidence has its own budget; run them together so a
    // slow gateway costs one budget per sync, not one per host.
    let ctx_ref = ctx;
    let captures = futures_util::future::join_all(hosts.iter().map(|host_id| async move {
        (
            *host_id,
            crate::feedback_capture::capture_host(host_id, ctx_ref).await,
        )
    }))
    .await;
    for (host_id, captured) in captures {
        match captured {
            Ok(outcome) => {
                if let Some(note) = outcome.pending_note() {
                    warnings.push(
                        HostWarningKind::EvidenceUnacknowledged,
                        host_id,
                        format!("Installation evidence pending: {note}"),
                    );
                }
            },
            Err(error) => {
                warnings.push(
                    HostWarningKind::EvidenceUnacknowledged,
                    host_id,
                    format!("Installation evidence unacknowledged: {error}"),
                );
                if matches!(
                    error,
                    crate::feedback::FeedbackError::Readback(_)
                        | crate::feedback::FeedbackError::Contract(_)
                ) {
                    report.host_failures.push(HostFailure {
                        host_id: HostId::new(host_id),
                        emitter: "installation-evidence".to_owned(),
                        error: format!("verify installed skill evidence: {error}"),
                        needs_elevation: false,
                    });
                }
            },
        }
    }
}
