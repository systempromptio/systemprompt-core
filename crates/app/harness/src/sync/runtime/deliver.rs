//! Posting what the outbox holds: due receipts first, then the pending session
//! bindings of every acknowledged, fully verified receipt, within 10 s. A
//! binding the instance does not take now defers its receipt's remaining
//! sessions. A refused device credential stops the delivery and is reported to
//! the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_device::client::ConsumerOutcome;
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::feedback::receipts::SessionBindingRequest;
use tokio::time::{Duration, Instant, timeout};

use crate::sync::cadence::{Clock, DELIVERY_BUDGET};
use crate::sync::outbox::{BindingAnswer, Outbox, ReceiptAnswer};
use crate::sync::ports::Instance;

const CONFLICT: u16 = 409;

pub(super) async fn deliver<I: Instance>(
    instance: &I,
    device: &DeviceCredential,
    outbox: &mut Outbox,
    clock: &dyn Clock,
) -> bool {
    let deadline = Instant::now() + DELIVERY_BUDGET;
    for index in outbox.due_receipts(clock.now()) {
        let Some(left) = remaining(deadline) else {
            return false;
        };
        let Some(receipt) = outbox.entries.get(index).map(|entry| entry.receipt.clone()) else {
            continue;
        };
        let answer = match timeout(left, instance.post_receipt(device, &receipt)).await {
            Ok(ConsumerOutcome::Acknowledged(response)) => ReceiptAnswer::Acknowledged {
                receipt_id: response.receipt_id,
                verified: response.fully_verified,
            },
            Ok(ConsumerOutcome::Refused(error)) => {
                tracing::warn!(error = %error, "the instance refused a receipt for good");
                if error.status() == Some(CONFLICT) {
                    ReceiptAnswer::Conflict
                } else {
                    ReceiptAnswer::Rejected
                }
            },
            Ok(ConsumerOutcome::DeviceRefused(_)) => return true,
            Ok(ConsumerOutcome::RetryLater(error)) => {
                tracing::debug!(error = %error, "a receipt waits for the next delivery");
                ReceiptAnswer::Retry
            },
            Err(_elapsed) => ReceiptAnswer::Retry,
        };
        outbox.settle_receipt(index, answer, clock.now());
    }
    deliver_bindings(instance, device, outbox, clock, deadline).await
}

async fn deliver_bindings<I: Instance>(
    instance: &I,
    device: &DeviceCredential,
    outbox: &mut Outbox,
    clock: &dyn Clock,
    deadline: Instant,
) -> bool {
    for index in 0..outbox.entries.len() {
        let Some(receipt_id) = outbox.binding_receipt(index, clock.now()) else {
            continue;
        };
        for session in outbox.pending_sessions(index) {
            let Some(left) = remaining(deadline) else {
                return false;
            };
            let request = SessionBindingRequest {
                receipt_id: receipt_id.clone(),
                host: HostKind::Harness.into(),
                session_id: session.clone(),
            };
            let answer = match timeout(left, instance.post_session_binding(device, &request)).await
            {
                Ok(ConsumerOutcome::Acknowledged(_)) => BindingAnswer::Bound,
                Ok(ConsumerOutcome::Refused(error)) => {
                    tracing::debug!(error = %error, "the instance refused a session binding for good");
                    BindingAnswer::Refused
                },
                Ok(ConsumerOutcome::DeviceRefused(_)) => return true,
                Ok(ConsumerOutcome::RetryLater(error)) => {
                    tracing::debug!(error = %error, "a session binding waits for the next delivery");
                    BindingAnswer::Retry
                },
                Err(_elapsed) => BindingAnswer::Retry,
            };
            outbox.settle_binding(index, &session, answer, clock.now());
            if answer == BindingAnswer::Retry {
                break;
            }
        }
    }
    false
}

fn remaining(deadline: Instant) -> Option<Duration> {
    Some(deadline.saturating_duration_since(Instant::now())).filter(|left| !left.is_zero())
}
