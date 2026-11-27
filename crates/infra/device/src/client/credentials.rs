//! The JWT contract between the client and whoever holds the account's
//! credentials.
//!
//! The client never holds a credential; a [`CredentialSource`] (implemented by
//! the accounts crate) hands out JWTs on demand. [`with_jwt`] runs one call
//! with a token, and when the instance refuses it, mints a fresh one and runs
//! the call once more. A fresh token refused again is reported to the source
//! with [`CredentialSource::rejected`], which is the signed-out signal; a
//! session mismatch earns the same single retry but is not a refusal of the
//! credential.
//!
//! A [`DeviceTokenSource`] (implemented by the same crate) hands out the
//! device token of the account's current sign-in, the credential the Drive sync
//! trades for its own sync token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::Duration;

use sp_protocol::Jwt;

use crate::client::error::ClientError;

const REUSE_MARGIN: Duration = Duration::from_secs(60);

/// How fresh a JWT the caller needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Any,
    ValidFor(Duration),
    Fresh,
}

/// Why no JWT could be handed out.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("no account is signed in")]
    SignedOut,
    #[error("the OS secure store does not open the credential")]
    Insecure,
    #[error("the instance refused the stored credential")]
    Refused,
    #[error("a session token could not be minted")]
    Mint(#[source] Box<ClientError>),
}

/// Hands out JWTs for one account. Implementations mint from the sealed PAT,
/// cache in memory and renew.
///
/// `jwt` answers with a token at least as fresh as asked (`Freshness::Fresh`
/// always mints a new one); `rejected` is told when the instance refused a
/// freshly minted token. It uses `#[async_trait]` because callers hold it as
/// `dyn CredentialSource`.
#[async_trait::async_trait]
pub trait CredentialSource: Send + Sync {
    async fn jwt(&self, freshness: Freshness) -> Result<Jwt, CredentialError>;

    async fn rejected(&self);
}

/// Hands out the device token of the account's current sign-in, the credential
/// the Drive sync trades for its own. Implemented by the accounts crate; the
/// token is kept in memory only.
///
/// `device_token` answers `None` when the sign-in returned no device token;
/// `device_token_refused` is told when the instance refused it in the trade for
/// a sync token. It uses `#[async_trait]` because the sync holds it as
/// `dyn DeviceTokenSource`.
#[async_trait::async_trait]
pub trait DeviceTokenSource: Send + Sync {
    async fn device_token(&self) -> Result<Option<Jwt>, CredentialError>;

    async fn device_token_refused(&self);
}

pub async fn with_jwt<S, T, F, Fut>(source: &S, mut call: F) -> Result<T, ClientError>
where
    S: CredentialSource + ?Sized,
    F: FnMut(Jwt) -> Fut,
    Fut: Future<Output = Result<T, ClientError>>,
{
    let jwt = source
        .jwt(Freshness::ValidFor(REUSE_MARGIN))
        .await
        .map_err(ClientError::Credential)?;
    match call(jwt).await {
        Err(error) if error.refuses_token() => {
            tracing::debug!(error = %error, "session token refused; minting a fresh one");
            let fresh = source
                .jwt(Freshness::Fresh)
                .await
                .map_err(ClientError::Credential)?;
            let second = call(fresh).await;
            if let Err(again) = &second
                && again.is_unauthorized()
            {
                tracing::warn!(error = %again, "a freshly minted session token was refused");
                source.rejected().await;
            }
            second
        },
        other => other,
    }
}
