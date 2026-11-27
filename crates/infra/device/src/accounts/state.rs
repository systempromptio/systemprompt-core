//! The memory-only state beside the store: PATs kept for this run, session
//! slots, instance summaries, the browser sign-in in progress and the last
//! account signed in to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use sp_protocol::ids::AccountId;
use sp_protocol::view::InstanceSummary;
use crate::secure::SealedString;
use tokio_util::sync::CancellationToken;

use crate::accounts::session::SessionSlot;
use crate::accounts::snapshot::{AccountsSnapshot, pending_offers, view};
use crate::accounts::store::{AccountRecord, StoreFile};

#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) store: StoreFile,
    pub(crate) volatile: HashMap<AccountId, SealedString>,
    pub(crate) sessions: HashMap<AccountId, SessionSlot>,
    pub(crate) summaries: HashMap<AccountId, InstanceSummary>,
    pub(crate) sign_in: Option<BrowserAttempt>,
    pub(crate) sequence: u64,
    pub(crate) last_signed_in: Option<AccountId>,
}

#[derive(Debug)]
pub(crate) struct BrowserAttempt {
    pub(crate) id: u64,
    pub(crate) cancel: CancellationToken,
    pub(crate) url: Option<String>,
}

impl State {
    pub(crate) fn credential(&self, record: &AccountRecord) -> Option<SealedString> {
        record
            .pat
            .clone()
            .or_else(|| self.volatile.get(&record.id).cloned())
    }

    pub(crate) fn current_credential(&self, account: &AccountId) -> Option<SealedString> {
        self.store
            .find(account)
            .and_then(|record| self.credential(record))
    }

    pub(crate) fn is_signed_in(&self, record: &AccountRecord) -> bool {
        record.pat.is_some() || self.volatile.contains_key(&record.id)
    }

    pub(crate) const fn next_sequence(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }

    pub(crate) fn snapshot(&self, secure_storage: bool) -> AccountsSnapshot {
        let accounts = &self.store.accounts;
        AccountsSnapshot {
            accounts: accounts
                .iter()
                .map(|record| view(record, self.is_signed_in(record)))
                .collect(),
            active: self.store.active().map(|record| record.id.clone()),
            offers: pending_offers(accounts),
            signing_in: self
                .sign_in
                .as_ref()
                .and_then(|attempt| attempt.url.clone()),
            last_signed_in: self.last_signed_in.clone(),
            secure_storage,
        }
    }
}
