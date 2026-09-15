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

pub async fn capture_host(host: &str, ctx: &HostSyncCtx<'_>) -> Result<()> {
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
        return Ok(());
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
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        crate::feedback::recover_pending(&enrollment, &outbox, kind, ctx.manifest),
    )
    .await??;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        crate::feedback::deliver(&enrollment, &outbox),
    )
    .await?
}
