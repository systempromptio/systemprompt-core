//! Account management: activating an account, signing it out, removing it, its
//! client and its instance key.
//!
//! Signing out forgets the account's PAT, session, device credential and outbox
//! pointer and keeps the row, signed out. When the account that leaves was
//! active, the first other signed-in account becomes active. Shutting the
//! accounts down stops every renewal task and any browser sign-in; the accounts
//! stay readable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use crate::client::SpClient;
use sp_protocol::ids::{AccountId, InstanceKey};

use crate::accounts::accounts::Accounts;
use crate::accounts::error::AccountsError;
use crate::accounts::slug::unique_slug;
use crate::accounts::store::StoreFile;

impl Accounts {
    pub fn activate(&self, id: &AccountId) -> Result<(), AccountsError> {
        self.mutate_store(|store, volatile| {
            let record = store.find_mut(id)?;
            if record.pat.is_none() && !volatile.contains(id) {
                return Err(AccountsError::NotSignedIn {
                    account: id.clone(),
                });
            }
            for record in &mut store.accounts {
                record.active = &record.id == id;
            }
            Ok(())
        })?;
        tracing::info!(account = %id, "account activated");
        self.publish();
        Ok(())
    }

    pub fn sign_out(&self, id: &AccountId) -> Result<(), AccountsError> {
        self.drop_session(id);
        {
            let mut state = self.inner.state.lock();
            state.volatile.remove(id);
            state.summaries.remove(id);
        }
        self.mutate_store(|store, volatile| {
            let record = store.find_mut(id)?;
            record.pat = None;
            record.device = None;
            record.outbox = None;
            let was_active = std::mem::replace(&mut record.active, false);
            if was_active {
                activate_successor(store, volatile, id);
            }
            Ok(())
        })?;
        tracing::info!(account = %id, "account signed out");
        self.publish();
        Ok(())
    }

    pub fn remove(&self, id: &AccountId) -> Result<(), AccountsError> {
        self.drop_session(id);
        {
            let mut state = self.inner.state.lock();
            state.volatile.remove(id);
            state.summaries.remove(id);
            if state.last_signed_in.as_ref() == Some(id) {
                state.last_signed_in = None;
            }
        }
        self.mutate_store(|store, volatile| {
            let index = store
                .accounts
                .iter()
                .position(|record| &record.id == id)
                .ok_or_else(|| AccountsError::UnknownAccount {
                    account: id.clone(),
                })?;
            let removed = store.accounts.remove(index);
            if removed.active {
                activate_successor(store, volatile, id);
            }
            Ok(())
        })?;
        self.inner.mint_locks.lock().remove(id);
        tracing::info!(account = %id, "account removed");
        self.publish();
        Ok(())
    }

    pub fn client(&self, id: &AccountId) -> Result<SpClient, AccountsError> {
        let url = self.read(id, |record, _| record.url.clone())?;
        SpClient::new(&url, self.inner.http.clone()).map_err(AccountsError::Client)
    }

    pub fn instance_key(&self, id: &AccountId) -> Result<InstanceKey, AccountsError> {
        if let Some(key) = self.read(id, |record, _| record.instance_key.clone())? {
            return Ok(key);
        }
        self.mutate_store(|store, _| {
            let label = store.find_mut(id)?.label.clone();
            let key = InstanceKey::new(unique_slug(&label, &store.taken_keys()));
            store.find_mut(id)?.instance_key = Some(key.clone());
            Ok(key)
        })
    }

    pub fn cancel_sign_in(&self) {
        if let Some(attempt) = self.inner.state.lock().sign_in.as_ref() {
            attempt.cancel.cancel();
        }
    }

    pub fn shutdown(&self) {
        self.inner.shutdown.cancel();
    }
}

fn activate_successor(store: &mut StoreFile, volatile: &HashSet<AccountId>, leaving: &AccountId) {
    if store.active().is_some() {
        return;
    }
    let successor = store.accounts.iter_mut().find(|record| {
        &record.id != leaving && (record.pat.is_some() || volatile.contains(&record.id))
    });
    if let Some(next) = successor {
        next.active = true;
        tracing::info!(account = %next.id, "the next signed-in account became active");
    }
}
