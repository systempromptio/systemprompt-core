//! Sessions waiting on the outbox's receipts: which entries a session binds
//! to, when a binding is ready to post and how its answer is recorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{InstallationReceiptId, ManagedResourceId, NativeSessionId};

use super::{Binding, BindingAnswer, Delivery, Entry, Outbox, SESSIONS_PER_ENTRY, defer};

impl Outbox {
    pub fn bind_session(&mut self, session: &NativeSessionId) {
        let live =
            |entry: &Entry| matches!(entry.delivery, Delivery::Pending | Delivery::Acknowledged);
        let mut newest: HashMap<ManagedResourceId, i64> = HashMap::new();
        for entry in self.entries.iter().filter(|entry| live(entry)) {
            let generation = newest
                .entry(entry.receipt.resource_id.clone())
                .or_insert(entry.receipt.generation);
            *generation = (*generation).max(entry.receipt.generation);
        }
        for entry in &mut self.entries {
            let current = newest.get(&entry.receipt.resource_id) == Some(&entry.receipt.generation);
            if !live(entry) || !current || entry.sessions.contains_key(session) {
                continue;
            }
            let waiting = entry
                .sessions
                .values()
                .filter(|state| **state == Binding::Pending)
                .count();
            if waiting >= SESSIONS_PER_ENTRY {
                continue;
            }
            while entry.sessions.len() >= SESSIONS_PER_ENTRY {
                let settled = entry
                    .sessions
                    .iter()
                    .find(|(_, state)| **state != Binding::Pending)
                    .map(|(key, _)| key.clone());
                match settled {
                    Some(key) => entry.sessions.remove(&key),
                    None => break,
                };
            }
            entry.sessions.insert(session.clone(), Binding::Pending);
        }
    }

    pub fn binding_receipt(
        &self,
        index: usize,
        now: DateTime<Utc>,
    ) -> Option<InstallationReceiptId> {
        let entry = self.entries.get(index)?;
        let ready =
            entry.delivery == Delivery::Acknowledged && entry.verified && entry.next_at <= now;
        if ready {
            entry.receipt_id.clone()
        } else {
            None
        }
    }

    pub fn pending_sessions(&self, index: usize) -> Vec<NativeSessionId> {
        self.entries.get(index).map_or_else(Vec::new, |entry| {
            entry
                .sessions
                .iter()
                .filter(|(_, state)| **state == Binding::Pending)
                .map(|(key, _)| key.clone())
                .collect()
        })
    }

    pub fn settle_binding(
        &mut self,
        index: usize,
        session: &NativeSessionId,
        answer: BindingAnswer,
        now: DateTime<Utc>,
    ) {
        let Some(entry) = self.entries.get_mut(index) else {
            return;
        };
        match answer {
            BindingAnswer::Bound => {
                entry.sessions.insert(session.clone(), Binding::Bound);
                entry.attempts = 0;
            },
            BindingAnswer::Refused => {
                entry.sessions.insert(session.clone(), Binding::Refused);
            },
            BindingAnswer::Retry => defer(entry, now),
        }
    }
}
