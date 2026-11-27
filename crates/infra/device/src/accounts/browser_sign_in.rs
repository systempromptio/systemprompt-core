//! The browser sign-in: a loopback listener, the instance's device-link page
//! opened with it as the redirect, and the one-time code it brings back
//! exchanged for a PAT.
//!
//! The sign-in runs when its outcome is awaited. Starting a second browser
//! sign-in, or `Accounts::cancel_sign_in`, cancels the one in progress, which
//! then resolves as `Cancelled`. While the page is open the snapshot names the
//! instance being signed in to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::pin::Pin;

use crate::client::{CallbackOutcome, LoopbackCallback};
use crate::protocol::OneTimeCode;
use crate::view::{SignInFailure, SignInOutcome};
use tokio_util::sync::CancellationToken;

use crate::accounts::accounts::Accounts;
use crate::accounts::signin::{Reached, until_cancelled};
use crate::accounts::state::BrowserAttempt;

type SignInRun = Pin<Box<dyn Future<Output = SignInOutcome> + Send + 'static>>;

/// Ends a browser sign-in in progress; the sign-in then resolves as
/// `Cancelled`.
#[derive(Debug, Clone)]
pub struct SignInCancel {
    token: CancellationToken,
}

impl SignInCancel {
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

/// A browser sign-in, started when awaited through [`BrowserSignIn::outcome`].
/// A second browser sign-in, or [`Accounts::cancel_sign_in`], cancels it.
pub struct BrowserSignIn {
    cancel: SignInCancel,
    run: SignInRun,
}

impl BrowserSignIn {
    #[must_use]
    pub fn canceller(&self) -> SignInCancel {
        self.cancel.clone()
    }

    pub async fn outcome(self) -> SignInOutcome {
        self.run.await
    }
}

impl fmt::Debug for BrowserSignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserSignIn")
            .field("cancelled", &self.cancel.token.is_cancelled())
            .finish_non_exhaustive()
    }
}

struct AttemptGuard {
    accounts: Accounts,
    attempt: u64,
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        let cleared = {
            let mut state = self.accounts.inner.state.lock();
            let ours = state
                .sign_in
                .as_ref()
                .is_some_and(|attempt| attempt.id == self.attempt);
            if ours {
                state.sign_in = None;
            }
            ours
        };
        if cleared {
            self.accounts.publish();
        }
    }
}

impl Accounts {
    #[must_use]
    pub fn sign_in_browser(&self, url: &str, keep: bool) -> BrowserSignIn {
        let token = self.inner.shutdown.child_token();
        let attempt = {
            let mut state = self.inner.state.lock();
            if let Some(previous) = state.sign_in.take() {
                previous.cancel.cancel();
            }
            let id = state.next_sequence();
            state.sign_in = Some(BrowserAttempt {
                id,
                cancel: token.clone(),
                url: None,
            });
            id
        };
        let accounts = self.clone();
        let url = url.to_owned();
        let cancel = token.clone();
        let run: SignInRun = Box::pin(async move {
            let _attempt = AttemptGuard {
                accounts: accounts.clone(),
                attempt,
            };
            let outcome = accounts.browser_flow(&url, keep, attempt, &cancel).await;
            tracing::info!(outcome = ?outcome, "browser sign-in ended");
            outcome
        });
        BrowserSignIn {
            cancel: SignInCancel { token },
            run,
        }
    }

    async fn browser_flow(
        &self,
        url: &str,
        keep: bool,
        attempt: u64,
        cancel: &CancellationToken,
    ) -> SignInOutcome {
        if let Some(refused) = self.refuse_without_secure_storage() {
            return refused;
        }
        let reached = match until_cancelled(cancel, self.reach(url)).await {
            None => return SignInOutcome::Cancelled,
            Some(Err(failure)) => return SignInOutcome::Failed(failure),
            Some(Ok(reached)) => reached,
        };
        let code = match self.browser_code(&reached, attempt, cancel).await {
            Ok(code) => code,
            Err(outcome) => return outcome,
        };
        let device_name = Some(self.inner.device_name.as_str());
        let exchange = reached.client.exchange_code(&code, device_name);
        let exchanged = until_cancelled(cancel, exchange).await;
        drop(code);
        let pat = match exchanged {
            None => return SignInOutcome::Cancelled,
            Some(Err(error)) => {
                tracing::info!(error = %error, "the one-time code was not exchanged");
                return SignInOutcome::Failed(error.sign_in_failure());
            },
            Some(Ok(pat)) => pat,
        };
        self.complete(&reached, pat, keep, Some(cancel)).await
    }

    async fn browser_code(
        &self,
        reached: &Reached,
        attempt: u64,
        cancel: &CancellationToken,
    ) -> Result<OneTimeCode, SignInOutcome> {
        let callback = LoopbackCallback::bind(&self.inner.product)
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "the loopback listener could not be started");
                SignInOutcome::Failed(SignInFailure::Failed)
            })?;
        let page = reached
            .discovery
            .device_link_url(Some(callback.redirect_url()))
            .map_err(|error| SignInOutcome::Failed(error.sign_in_failure()))?;
        if cancel.is_cancelled() {
            return Err(SignInOutcome::Cancelled);
        }
        self.mark_signing_in(attempt, reached.client.base());
        self.inner.browser.open(&page).map_err(|error| {
            tracing::warn!(error = %error, "the device-link page could not be opened");
            SignInOutcome::Failed(SignInFailure::Failed)
        })?;
        let answer = match until_cancelled(cancel, callback.wait()).await {
            None => return Err(SignInOutcome::Cancelled),
            Some(Err(error)) => {
                tracing::warn!(error = %error, "the loopback listener failed");
                return Err(SignInOutcome::Failed(SignInFailure::Failed));
            },
            Some(Ok(answer)) => answer,
        };
        match answer {
            CallbackOutcome::Code(code) => Ok(code),
            CallbackOutcome::Denied => Err(SignInOutcome::Denied),
            CallbackOutcome::Expired => Err(SignInOutcome::Expired),
            CallbackOutcome::Cancelled => Err(SignInOutcome::Cancelled),
        }
    }

    fn mark_signing_in(&self, attempt: u64, url: &str) {
        let marked = {
            let mut state = self.inner.state.lock();
            match state.sign_in.as_mut() {
                Some(current) if current.id == attempt => {
                    current.url = Some(url.to_owned());
                    true
                },
                _ => false,
            }
        };
        if marked {
            self.publish();
        }
    }
}
