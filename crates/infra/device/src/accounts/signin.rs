//! Signing in with a pasted one-time code or a personal access token, and the
//! steps every sign-in flow shares.
//!
//! No flow returns an error: each ends in a `SignInOutcome` carrying only the
//! kind of failure, never the client's error. No sign-in starts while the OS
//! secure store is unavailable. The code page is the instance's device-link
//! page opened without a redirect; it shows the one-time code the user pastes
//! back. With `keep == false` the PAT is held sealed in memory for this run and
//! never written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::client::{InstanceDiscovery, SpClient};
use crate::protocol::{OneTimeCode, Pat};
use crate::view::{SignInFailure, SignInOutcome};
use tokio_util::sync::CancellationToken;

use crate::accounts::accounts::Accounts;

pub(crate) struct Reached {
    pub(crate) client: SpClient,
    pub(crate) discovery: InstanceDiscovery,
}

impl Accounts {
    pub async fn open_code_page(&self, url: &str) -> Result<(), SignInFailure> {
        let client =
            SpClient::new(url, self.inner.http.clone()).map_err(|error| error.sign_in_failure())?;
        let discovery = client
            .discover()
            .await
            .map_err(|error| error.sign_in_failure())?;
        let page = discovery
            .device_link_url(None)
            .map_err(|error| error.sign_in_failure())?;
        self.inner.browser.open(&page).map_err(|error| {
            tracing::warn!(error = %error, "the device-link page could not be opened");
            SignInFailure::Failed
        })
    }

    pub async fn sign_in_code(&self, url: &str, code: String, keep: bool) -> SignInOutcome {
        if let Some(refused) = self.refuse_without_secure_storage() {
            return refused;
        }
        let Ok(code) = OneTimeCode::new(code) else {
            return SignInOutcome::Failed(SignInFailure::Refused);
        };
        let reached = match self.reach(url).await {
            Ok(reached) => reached,
            Err(failure) => return SignInOutcome::Failed(failure),
        };
        let device_name = Some(self.inner.device_name.as_str());
        let pat = match reached.client.exchange_code(&code, device_name).await {
            Ok(pat) => pat,
            Err(error) => {
                tracing::info!(error = %error, "the one-time code was not exchanged");
                return SignInOutcome::Failed(error.sign_in_failure());
            },
        };
        drop(code);
        let outcome = self.complete(&reached, pat, keep, None).await;
        tracing::info!(outcome = ?outcome, "code sign-in ended");
        outcome
    }

    pub async fn sign_in_pat(&self, url: &str, pat: String, keep: bool) -> SignInOutcome {
        if let Some(refused) = self.refuse_without_secure_storage() {
            return refused;
        }
        let Ok(pat) = Pat::new(pat) else {
            return SignInOutcome::Failed(SignInFailure::Refused);
        };
        let reached = match self.reach(url).await {
            Ok(reached) => reached,
            Err(failure) => return SignInOutcome::Failed(failure),
        };
        let outcome = self.complete(&reached, pat, keep, None).await;
        tracing::info!(outcome = ?outcome, "token sign-in ended");
        outcome
    }

    pub(crate) async fn reach(&self, url: &str) -> Result<Reached, SignInFailure> {
        let client =
            SpClient::new(url, self.inner.http.clone()).map_err(|error| error.sign_in_failure())?;
        let discovery = client.discover().await.map_err(|error| {
            tracing::info!(error = %error, "the instance was not reached");
            error.sign_in_failure()
        })?;
        Ok(Reached { client, discovery })
    }

    pub(crate) fn refuse_without_secure_storage(&self) -> Option<SignInOutcome> {
        if self.inner.sealer.available() {
            return None;
        }
        tracing::warn!("sign-in refused: the OS secure store is unavailable");
        Some(SignInOutcome::Failed(SignInFailure::Insecure))
    }
}

pub(crate) async fn until_cancelled<F: Future>(
    cancel: &CancellationToken,
    work: F,
) -> Option<F::Output> {
    tokio::select! {
        () = cancel.cancelled() => None,
        output = work => Some(output),
    }
}
