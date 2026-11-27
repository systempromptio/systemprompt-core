//! What the shell renders from: every account as the UI sees it, the active
//! one, the key offers waiting for the user and the browser sign-in in
//! progress. Built from the store and memory; it never holds a secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_protocol::ids::AccountId;
use sp_protocol::view::{AccountStatus, AccountView, KeyOffer};

use crate::accounts::keys;
use crate::accounts::store::AccountRecord;

/// The accounts as the shell shows them, published on every change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccountsSnapshot {
    pub accounts: Vec<AccountView>,
    pub active: Option<AccountId>,
    pub offers: Vec<PendingKeyOffer>,
    pub signing_in: Option<String>,
    pub last_signed_in: Option<AccountId>,
    pub secure_storage: bool,
}

/// A manifest key an account's instance serves and the user has not trusted
/// yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingKeyOffer {
    pub account: AccountId,
    pub offer: KeyOffer,
}

impl AccountsSnapshot {
    #[must_use]
    pub fn account(&self, id: &AccountId) -> Option<&AccountView> {
        self.accounts.iter().find(|account| &account.id == id)
    }

    #[must_use]
    pub fn active_account(&self) -> Option<&AccountView> {
        self.active.as_ref().and_then(|id| self.account(id))
    }

    #[must_use]
    pub fn needs_sign_in(&self) -> bool {
        self.active_account()
            .is_none_or(|account| account.status != AccountStatus::SignedIn)
    }
}

pub(crate) fn view(record: &AccountRecord, signed_in: bool) -> AccountView {
    let offered = keys::pending(record).map(|offer| offer.offered);
    let trusted = offered.is_none() && keys::trusted_key(record).is_some();
    AccountView {
        id: record.id.clone(),
        label: record.label.clone(),
        url: record.url.clone(),
        user: record.user.clone(),
        org: record.org.clone(),
        status: if signed_in {
            AccountStatus::SignedIn
        } else {
            AccountStatus::SignedOut
        },
        active: record.active,
        fingerprint: offered.or_else(|| record.pinned_fingerprint.clone()),
        pinned: record.pinned_fingerprint.clone(),
        trusted,
    }
}

pub(crate) fn pending_offers<'a>(
    records: impl IntoIterator<Item = &'a AccountRecord>,
) -> Vec<PendingKeyOffer> {
    records
        .into_iter()
        .filter_map(|record| {
            keys::pending(record).map(|offer| PendingKeyOffer {
                account: record.id.clone(),
                offer,
            })
        })
        .collect()
}
