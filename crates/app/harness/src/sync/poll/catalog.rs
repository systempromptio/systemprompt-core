//! The poller's side reads: the profile and model catalog, and the served
//! manifest key offered for trust when the pinned one no longer verifies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_client::{CredentialSource, with_jwt};

use super::Poller;
use crate::ports::{Instance, Store};
use crate::target::Target;

impl<S: Store, T: Target> Poller<S, T> {
    pub(super) async fn offer_served_key(&self) {
        let account = &self.scope.account;
        match self.scope.instance.pubkey().await {
            Ok(key) => {
                if let Err(error) = self.scope.store.offer_key(account, &key) {
                    tracing::warn!(account = %account, error = %error, "recording the served manifest key failed");
                } else {
                    tracing::info!(account = %account, offered = %key.fingerprint(), "sync stopped until the served manifest key is trusted");
                }
            },
            Err(error) => {
                tracing::warn!(account = %account, error = %error, "reading the served manifest key failed");
            },
        }
    }

    pub(super) async fn read_catalog(&mut self, source: &dyn CredentialSource) {
        let instance = &self.scope.instance;
        let (profile, catalog) = tokio::join!(
            with_jwt(
                source,
                move |jwt| async move { instance.profile(&jwt).await }
            ),
            with_jwt(
                source,
                move |jwt| async move { instance.models(&jwt).await }
            ),
        );
        match profile {
            Ok(profile) => self.profile = Some(profile),
            Err(error) => {
                tracing::debug!(error = %error, "reading the profile failed; keeping the previous one");
            },
        }
        match catalog {
            Ok(catalog) => self.catalog = Some(catalog),
            Err(error) => {
                tracing::debug!(error = %error, "reading the model catalog failed; keeping the previous one");
            },
        }
    }
}
