//! Captures a host's installed managed skills into the feedback outbox after a
//! sync applies them; sits above `host_sync` and `integration` because it
//! resolves each host's on-disk skill roots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod hosts;

use crate::feedback::credentials::Enrollment;
use crate::feedback::outbox::Outbox;
use crate::feedback::{FeedbackError, Result};
use crate::host_sync::HostSyncCtx;

const RECOVERY_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
const DELIVERY_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

/// One host's evidence capture, counted.
///
/// How many due installation receipts were produced, how many are still
/// pending, and whether the queued receipts reached the gateway in budget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CaptureOutcome {
    pub recovered: usize,
    pub remaining: usize,
    pub undelivered: bool,
}

impl CaptureOutcome {
    #[must_use]
    pub fn pending_note(&self) -> Option<String> {
        let total = self.recovered + self.remaining;
        match (self.remaining, self.undelivered) {
            (0, false) => None,
            (0, true) => Some(
                "installation receipts are queued but not yet delivered; retried on the next sync"
                    .to_owned(),
            ),
            (n, _) => Some(format!(
                "{n} of {total} installation receipts still pending after {}s; retried on the next sync",
                RECOVERY_BUDGET.as_secs()
            )),
        }
    }
}

pub async fn capture_host(host: &str, ctx: &HostSyncCtx<'_>) -> Result<CaptureOutcome> {
    let kind = crate::feedback::client_kind(host).ok_or(FeedbackError::Scope)?;
    let skills: Vec<_> = ctx
        .manifest
        .skills
        .iter()
        .filter(|skill| {
            skill.publication.is_some()
                && (skill.hosts.is_empty()
                    || skill
                        .hosts
                        .iter()
                        .any(|host| crate::feedback::client_kind(host) == Some(kind)))
        })
        .collect();
    if skills.is_empty() {
        return Ok(CaptureOutcome::default());
    }
    let root = crate::feedback::metadata_root()?;
    let enrollment = Enrollment::load(&root, ctx.client.base_url_str())?;
    if enrollment.consumer_id != ctx.manifest.user_id {
        return Err(FeedbackError::Scope);
    }
    let outbox = Outbox::new(
        enrollment.outbox_path(&root),
        crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
    );
    for skill in skills {
        let publication = skill.publication.as_ref().ok_or(FeedbackError::Scope)?;
        if outbox.has_acknowledged_receipt(kind, publication)? {
            continue;
        }
        let roots = hosts::roots(host, ctx, skill)?;
        if roots.is_empty() {
            return Err(FeedbackError::HostUnavailable);
        }
        outbox.reserve_installation(crate::feedback::outbox::PendingInstallation::new(
            skill.publication.clone().ok_or(FeedbackError::Scope)?,
            kind,
            roots,
        ))?;
    }
    let deadline = std::time::Instant::now() + RECOVERY_BUDGET;
    let progress =
        crate::feedback::recover_pending(&enrollment, &outbox, kind, ctx.manifest, deadline)
            .await?;
    // Why: the deadline is the budget, not a fault; a queue that did not
    // drain in time is reported as pending and retried by the next pass.
    let undelivered = match tokio::time::timeout(
        DELIVERY_BUDGET,
        crate::feedback::deliver(&enrollment, &outbox),
    )
    .await
    {
        Ok(result) => {
            result?;
            false
        },
        Err(_elapsed) => true,
    };
    Ok(CaptureOutcome {
        recovered: progress.recovered,
        remaining: progress.remaining,
        undelivered,
    })
}
