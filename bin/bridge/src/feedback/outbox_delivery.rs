//! Delivery state of queued receipts: recording gateway outcomes and
//! answering whether evidence is already acknowledged or refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Delivery, FeedbackError, Outbox, Result, State, retry_after};
use chrono::Utc;
use systemprompt_models::feedback::receipts::ConsumerReceiptResponse;

impl Outbox {
    // Why: a receipt the gateway has already acknowledged for this exact
    // publication is evidence on file; re-planning it on every sync is one
    // bundle round-trip per skill per host for nothing.
    pub fn has_acknowledged_receipt(
        &self,
        host: systemprompt_models::feedback::EvaluatorClient,
        publication: &systemprompt_models::bridge::manifest::ManagedPublicationRef,
    ) -> Result<bool> {
        self.read_locked(|state| {
            Ok(state.entries.values().any(|entry| {
                matches!(entry.delivery, Delivery::Acknowledged(_))
                    && entry.request.host == host
                    && entry.request.publication_id == publication.publication_id
                    && entry.request.revision_id == publication.revision_id
                    && entry.request.generation == publication.generation
                    && entry.request.bundle_digest.as_str() == publication.bundle_digest.as_str()
            }))
        })
    }

    pub fn delivery(
        &self,
        key: &str,
        result: std::result::Result<ConsumerReceiptResponse, u16>,
    ) -> Result<()> {
        self.mutate(|state| {
            let entry = state
                .entries
                .get_mut(key)
                .ok_or(FeedbackError::EntryMissing)?;
            if matches!(entry.delivery, Delivery::Acknowledged(_)) {
                return Ok(());
            }
            let status = match result {
                Ok(response) => {
                    entry.delivery = Delivery::Acknowledged(response);
                    entry.attempts = 0;
                    entry.next_attempt = Utc::now();
                    return Ok(());
                },
                Err(status) => status,
            };
            entry.attempts = entry.attempts.saturating_add(1);
            entry.delivery = match status {
                409 => Delivery::Conflict,
                401 => Delivery::CredentialRejected,
                // Why: the gateway's 403 says this identity may not file
                // consumer evidence at all; retrying a permanent refusal only
                // grew the outbox, so it is terminal and evictable.
                400 | 403 | 404 | 422 => Delivery::Rejected(status),
                _ => Delivery::Unacknowledged,
            };
            entry.next_attempt = Utc::now() + retry_after(entry.attempts);
            Ok(())
        })
    }
}

pub(super) fn refused_anywhere(
    state: &State,
    host: systemprompt_models::feedback::EvaluatorClient,
    session: &str,
) -> bool {
    state
        .entries
        .values()
        .any(|entry| entry.request.host == host && entry.refused_sessions.contains_key(session))
}
