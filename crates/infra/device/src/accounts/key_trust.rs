//! The manifest key operations of the accounts API, over the key state machine
//! in `keys`.
//!
//! The pinned key is `None` while nothing is trusted or while a different key
//! waits for the user, which stops manifest use.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::client::ManifestKey;
use sp_protocol::ids::AccountId;
use sp_protocol::view::{KeyFingerprint, KeyOffer};

use crate::accounts::accounts::Accounts;
use crate::accounts::error::AccountsError;
use crate::accounts::keys;

impl Accounts {
    pub fn offer_key(&self, id: &AccountId, key: &ManifestKey) -> Result<(), AccountsError> {
        let changed = self.mutate_store(|store, _| Ok(keys::offer(store.find_mut(id)?, key)))?;
        if changed {
            tracing::info!(account = %id, offered = %key.fingerprint(), "manifest key offered");
            self.publish();
        }
        Ok(())
    }

    pub fn pinned_key(&self, id: &AccountId) -> Result<Option<ManifestKey>, AccountsError> {
        self.read(id, |record, _| keys::trusted_key(record))
    }

    pub fn key_offer(&self, id: &AccountId) -> Result<Option<KeyOffer>, AccountsError> {
        self.read(id, |record, _| keys::pending(record))
    }

    pub fn trust(&self, id: &AccountId, fingerprint: &KeyFingerprint) -> Result<(), AccountsError> {
        self.mutate_store(|store, _| {
            if keys::trust(store.find_mut(id)?, fingerprint) {
                Ok(())
            } else {
                Err(AccountsError::NothingToTrust {
                    account: id.clone(),
                })
            }
        })?;
        tracing::info!(account = %id, pinned = %fingerprint, "manifest key trusted");
        self.publish();
        Ok(())
    }
}
