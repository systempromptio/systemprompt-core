//! The device token of an account's sign-in, handed to the Drive sync through
//! [`sp_client::DeviceTokenSource`].
//!
//! The instance returns it beside every session token minted from the account's
//! PAT, so it lives where the session token lives: in memory, in the account's
//! held session, renewed with it. A token the instance refused is replaced by
//! minting a fresh session on the next request. `DeviceTokenSource` is used as
//! `dyn`, hence `#[async_trait]`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use sp_client::{CredentialError, DeviceTokenSource, Freshness};
use sp_protocol::Jwt;
use sp_protocol::ids::AccountId;

use crate::accounts::Accounts;

impl Accounts {
    #[must_use]
    pub fn device_tokens(&self, account: &AccountId) -> Arc<dyn DeviceTokenSource> {
        Arc::new(AccountDeviceTokens {
            accounts: self.clone(),
            account: account.clone(),
            refused: AtomicBool::new(false),
        })
    }
}

struct AccountDeviceTokens {
    accounts: Accounts,
    account: AccountId,
    refused: AtomicBool,
}

#[async_trait]
impl DeviceTokenSource for AccountDeviceTokens {
    async fn device_token(&self) -> Result<Option<Jwt>, CredentialError> {
        let refused = self.refused.swap(false, Ordering::AcqRel);
        let freshness = if refused {
            Freshness::Fresh
        } else {
            Freshness::Any
        };
        let session = self.accounts.session(&self.account, freshness).await?;
        Ok(session.device_token)
    }

    async fn device_token_refused(&self) {
        tracing::info!(
            account = %self.account,
            "the instance refused the device token; the next one comes with a fresh session"
        );
        self.refused.store(true, Ordering::Release);
    }
}
