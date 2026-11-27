//! The receipts outbox of one account's device.
//!
//! One entry per installation and publication, a newer generation of
//! a resource superseding the older, each entry acknowledged, refused for good
//! (a conflict or a rejection) or tried again after 2^attempts seconds, at most
//! an hour. Sessions governance reported with the device evidence wait on the
//! entries of the newest generation of each resource (at most 256 waiting per
//! entry) and are posted once their entry is acknowledged and fully verified.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod file;
mod sessions;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sp_protocol::ids::{DeviceId, InstallationReceiptId, NativeSessionId};
use sp_protocol::wire::consumer::ReceiptRequest;
use sp_protocol::wire::manifest::Publication;

use crate::cadence::{after, outbox_delay};

pub use file::{OutboxError, file_name, load, outbox_path, save};

pub const OUTBOX_LIMIT: usize = 512;
pub const SESSIONS_PER_ENTRY: usize = 256;
const FORMAT: u32 = 1;

/// Where a receipt stands with the instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Pending,
    Acknowledged,
    Conflict,
    Rejected,
}

/// Where a session's binding to a receipt stands with the instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    Pending,
    Bound,
    Refused,
}

/// One receipt in the outbox, its delivery state and the sessions waiting on
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub receipt: ReceiptRequest,
    pub delivery: Delivery,
    pub attempts: u32,
    pub next_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<InstallationReceiptId>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub sessions: BTreeMap<NativeSessionId, Binding>,
}

/// The receipts outbox of one account's device, as it is kept on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outbox {
    format: u32,
    pub device: DeviceId,
    #[serde(default)]
    pub entries: Vec<Entry>,
}

/// How the instance answered a posted receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptAnswer {
    Acknowledged {
        receipt_id: InstallationReceiptId,
        verified: bool,
    },
    Conflict,
    Rejected,
    Retry,
}

/// How the instance answered a posted session binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingAnswer {
    Bound,
    Refused,
    Retry,
}

impl Outbox {
    pub const fn new(device: DeviceId) -> Self {
        Self {
            format: FORMAT,
            device,
            entries: Vec::new(),
        }
    }

    pub fn holds(&self, publication: &Publication) -> bool {
        self.entries
            .iter()
            .any(|entry| same_publication(&entry.receipt, publication))
    }

    pub fn enqueue(&mut self, receipt: ReceiptRequest, now: DateTime<Utc>) {
        let existing = self
            .entries
            .iter()
            .position(|entry| same_installation(&entry.receipt, &receipt));
        if let Some(index) = existing {
            let held = &self.entries[index].receipt;
            let unchanged = ReceiptRequest {
                observed_at: held.observed_at,
                ..receipt.clone()
            };
            if &unchanged == held {
                return;
            }
            self.entries.remove(index);
        }
        self.entries.retain(|entry| {
            !(entry.receipt.resource_id == receipt.resource_id
                && entry.receipt.generation < receipt.generation)
        });
        if self.entries.len() >= OUTBOX_LIMIT
            && let Some(settled) = self
                .entries
                .iter()
                .position(|entry| entry.delivery != Delivery::Pending)
        {
            self.entries.remove(settled);
        }
        self.entries.push(Entry {
            receipt,
            delivery: Delivery::Pending,
            attempts: 0,
            next_at: now,
            receipt_id: None,
            verified: false,
            sessions: BTreeMap::new(),
        });
    }

    pub fn due_receipts(&self, now: DateTime<Utc>) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&index| {
                let entry = &self.entries[index];
                entry.delivery == Delivery::Pending && entry.next_at <= now
            })
            .collect()
    }

    pub fn settle_receipt(&mut self, index: usize, answer: ReceiptAnswer, now: DateTime<Utc>) {
        let Some(entry) = self.entries.get_mut(index) else {
            return;
        };
        match answer {
            ReceiptAnswer::Acknowledged {
                receipt_id,
                verified,
            } => {
                entry.delivery = Delivery::Acknowledged;
                entry.attempts = 0;
                entry.next_at = now;
                entry.receipt_id = Some(receipt_id);
                entry.verified = verified;
            },
            ReceiptAnswer::Conflict => {
                entry.delivery = Delivery::Conflict;
                entry.attempts = entry.attempts.saturating_add(1);
            },
            ReceiptAnswer::Rejected => {
                entry.delivery = Delivery::Rejected;
                entry.attempts = entry.attempts.saturating_add(1);
            },
            ReceiptAnswer::Retry => defer(entry, now),
        }
    }

    pub fn undelivered(&self) -> bool {
        self.entries.iter().any(|entry| {
            entry.delivery == Delivery::Pending
                || (entry.delivery == Delivery::Acknowledged
                    && entry.verified
                    && entry.receipt_id.is_some()
                    && entry
                        .sessions
                        .values()
                        .any(|state| *state == Binding::Pending))
        })
    }
}

fn defer(entry: &mut Entry, now: DateTime<Utc>) {
    entry.attempts = entry.attempts.saturating_add(1);
    entry.next_at = after(now, outbox_delay(entry.attempts));
}

fn same_publication(receipt: &ReceiptRequest, publication: &Publication) -> bool {
    receipt.publication_id == publication.publication_id
        && receipt.resource_id == publication.resource_id
        && receipt.revision_id == publication.revision_id
        && receipt.generation == publication.generation
        && receipt.bundle_digest == publication.bundle_digest
}

fn same_installation(held: &ReceiptRequest, incoming: &ReceiptRequest) -> bool {
    held.installation_id == incoming.installation_id
        && held.resource_id == incoming.resource_id
        && held.publication_id == incoming.publication_id
        && held.host == incoming.host
}
