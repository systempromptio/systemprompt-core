//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials::Enrollment;
use super::outbox::{Delivery, Outbox};
use super::{FeedbackError, Result};
use systemprompt_identifiers::NativeSessionId;
use systemprompt_models::feedback::receipts::SessionBindingRequest;

pub async fn retry_pending(gateway: &str) -> Result<()> {
    let root = super::metadata_root()?;
    let enrollment = Enrollment::load(&root, gateway)?;
    let outbox = Outbox::new(
        enrollment.outbox_path(&root),
        crate::feedback::outbox::OutboxScope::from_enrollment(&enrollment),
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        deliver(&enrollment, &outbox),
    )
    .await?
}

pub async fn deliver(enrollment: &Enrollment, outbox: &Outbox) -> Result<()> {
    outbox.require_enrollment(enrollment)?;
    let mut failure = None;
    for (key, entry) in outbox
        .entries()?
        .into_iter()
        .filter(|(_, entry)| match &entry.delivery {
            Delivery::Unacknowledged | Delivery::CredentialRejected => {
                entry.next_attempt <= chrono::Utc::now()
            },
            Delivery::Acknowledged(response) => {
                response.fully_verified && entry.session_bindings.values().any(|bound| !bound)
            },
            Delivery::Conflict => false,
        })
        .take(64)
    {
        let response = match entry.delivery {
            Delivery::Conflict => {
                failure = Some(FeedbackError::Rejected(409));
                continue;
            },
            Delivery::CredentialRejected | Delivery::Unacknowledged => {
                if entry.next_attempt > chrono::Utc::now() {
                    continue;
                }
                match super::transport::receipt(enrollment, &entry.request).await {
                    Ok(response) => {
                        outbox.delivery(&key, Ok(response.clone()))?;
                        response
                    },
                    Err(error) => {
                        let status = match error {
                            FeedbackError::Rejected(status) => status,
                            _ => 0,
                        };
                        outbox.delivery(&key, Err(status))?;
                        failure = Some(error);
                        continue;
                    },
                }
            },
            Delivery::Acknowledged(response) => response,
        };
        if !response.fully_verified {
            continue;
        }
        for (session, bound) in entry.session_bindings {
            if bound {
                continue;
            }
            let request = SessionBindingRequest {
                receipt_id: response.receipt_id.clone(),
                host: entry.request.host,
                session_id: NativeSessionId::new(&session),
            };
            match super::transport::bind(enrollment, &request).await {
                Ok(()) => outbox.acknowledge_session(&key, &response.receipt_id, &session)?,
                Err(error) => {
                    failure = Some(error);
                    break;
                },
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

async fn recover_installation(
    enrollment: &Enrollment,
    outbox: &Outbox,
    pending: &super::outbox::PendingInstallation,
) -> Result<()> {
    let plan = super::transport::plan(enrollment, &pending.publication, pending.host).await?;
    let mut receipt = None;
    for root in &pending.roots {
        super::readback::materialize(root, &plan)?;
        receipt = Some(super::readback::verify(
            root,
            &plan,
            enrollment.installation_id.clone(),
        )?);
    }
    outbox.enqueue(receipt.ok_or(FeedbackError::Readback)?)?;
    Ok(())
}

pub async fn recover_pending(
    enrollment: &Enrollment,
    outbox: &Outbox,
    host: systemprompt_models::feedback::EvaluatorClient,
    manifest: &crate::gateway::manifest::SignedManifest,
) -> Result<()> {
    let mut failure = None;
    for (key, pending) in outbox
        .pending_installations()?
        .into_iter()
        .filter(|(_, pending)| {
            !pending.superseded
                && current_installation(manifest, pending)
                && pending.host == host
                && pending.next_attempt <= chrono::Utc::now()
        })
        .take(16)
    {
        outbox.begin_installation_attempt(&key)?;
        match recover_installation(enrollment, outbox, &pending).await {
            Ok(()) => outbox.complete_installation(&key)?,
            Err(error) => {
                failure = Some(error);
            },
        }
    }
    failure.map_or(Ok(()), Err)
}

fn current_installation(
    manifest: &crate::gateway::manifest::SignedManifest,
    pending: &super::outbox::PendingInstallation,
) -> bool {
    manifest
        .enabled_hosts
        .iter()
        .any(|host| super::client_kind(host) == Some(pending.host))
        && manifest.skills.iter().any(|skill| {
            (skill.hosts.is_empty()
                || skill
                    .hosts
                    .iter()
                    .any(|host| super::client_kind(host) == Some(pending.host)))
                && skill.publication.as_ref().is_some_and(|publication| {
                    publication.publication_id == pending.publication.publication_id
                        && publication.resource_id == pending.publication.resource_id
                        && publication.revision_id == pending.publication.revision_id
                        && publication.generation == pending.publication.generation
                        && publication.bundle_digest == pending.publication.bundle_digest
                })
        })
}

pub async fn recover_current_manifest(
    gateway: &str,
    manifest: &crate::gateway::manifest::SignedManifest,
) -> Result<()> {
    let root = super::metadata_root()?;
    let enrollment = Enrollment::load(&root, gateway)?;
    let outbox = Outbox::new(
        enrollment.outbox_path(&root),
        super::outbox::OutboxScope::from_enrollment(&enrollment),
    );
    recover_manifest_installations(&enrollment, &outbox, manifest).await
}

pub async fn recover_manifest_installations(
    enrollment: &Enrollment,
    outbox: &Outbox,
    manifest: &crate::gateway::manifest::SignedManifest,
) -> Result<()> {
    outbox.require_enrollment(enrollment)?;
    if enrollment.consumer_id != manifest.user_id {
        return Err(FeedbackError::Scope);
    }
    let _lock = super::installation_lock().await?;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        for host in &manifest.enabled_hosts {
            if let Some(kind) = super::client_kind(host) {
                recover_pending(enrollment, outbox, kind, manifest).await?;
            }
        }
        Ok(())
    })
    .await?
}
