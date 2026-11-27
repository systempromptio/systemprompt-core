//! The background renewal of an account's session token.
//!
//! One renewal task runs per signed-in account, owned by its session slot:
//! dropping the slot (sign out, removal, the accounts going away) aborts it,
//! and its cancellation token is a child of the accounts' shutdown token. It
//! renews at the time `schedule` gives, retries a failed renewal
//! while the token still holds, drops a token that expired, and ends when the
//! account is signed out or its PAT is refused. It holds the accounts weakly,
//! so it never keeps them alive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Weak};
use std::time::Duration;

use chrono::{DateTime, Utc};
use harness_tasks::OwnedTask;
use sp_client::{CredentialError, JwtSession};
use sp_protocol::ids::AccountId;
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;

use crate::accounts::{Accounts, Inner};
use crate::schedule::{renewal_delay, retry_delay};

const RENEWAL_TASK: &str = "session-renewal";

#[derive(Debug)]
pub(crate) struct Renewal {
    generation: u64,
    cancel: CancellationToken,
    task: Option<OwnedTask<()>>,
}

impl Renewal {
    pub(crate) fn is_running(&self) -> bool {
        !self.cancel.is_cancelled()
    }

    pub(crate) fn stop(self) {
        self.cancel.cancel();
        if let Some(task) = self.task {
            task.abort();
        }
    }
}

struct RenewalPlan {
    planned_for: DateTime<Utc>,
    wait: Duration,
}

impl Accounts {
    pub(crate) fn start_renewal(&self, account: &AccountId, generation: u64) -> Renewal {
        let cancel = self.inner.shutdown.child_token();
        let work = renewal_loop(
            Arc::downgrade(&self.inner),
            account.clone(),
            generation,
            cancel.clone(),
        );
        let task = self
            .inner
            .runtime
            .clone()
            .or_else(|| Handle::try_current().ok())
            .map_or_else(
                || {
                    tracing::warn!("no tokio runtime to renew session tokens on");
                    None
                },
                |runtime| {
                    let _entered = runtime.enter();
                    Some(OwnedTask::spawn(RENEWAL_TASK, work))
                },
            );
        Renewal {
            generation,
            cancel,
            task,
        }
    }

    async fn renew_now(&self, account: &AccountId) -> Result<JwtSession, CredentialError> {
        let lock = self.mint_lock(account);
        let _minting = lock.lock().await;
        self.mint(account).await
    }

    fn renewal_plan(&self, account: &AccountId) -> Option<RenewalPlan> {
        let state = self.inner.state.lock();
        let plan = state
            .sessions
            .get(account)
            .and_then(|slot| slot.held.as_ref())
            .map(|held| RenewalPlan {
                planned_for: held.minted_at,
                wait: renewal_delay(held.minted_at, held.session.expires_at, Utc::now()),
            });
        drop(state);
        plan
    }

    async fn renewal_step(
        &self,
        account: &AccountId,
        planned_for: DateTime<Utc>,
    ) -> Option<RenewalPlan> {
        let current = self.renewal_plan(account)?;
        if current.planned_for != planned_for {
            return Some(current);
        }
        match self.renew_now(account).await {
            Ok(_) => self.renewal_plan(account),
            Err(CredentialError::SignedOut | CredentialError::Refused) => None,
            Err(error) => self.retry_plan(account, planned_for, &error),
        }
    }

    fn retry_plan(
        &self,
        account: &AccountId,
        planned_for: DateTime<Utc>,
        error: &CredentialError,
    ) -> Option<RenewalPlan> {
        let expires_at = self.held_expiry(account);
        if let Some(wait) = expires_at.and_then(|expires_at| retry_delay(expires_at, Utc::now())) {
            tracing::debug!(
                account = %account,
                error = %error,
                retry_in = ?wait,
                "session renewal failed; retrying"
            );
            return Some(RenewalPlan { planned_for, wait });
        }
        tracing::warn!(
            account = %account,
            error = %error,
            "session renewal failed and the token expired"
        );
        self.drop_expired(account);
        None
    }

    fn held_expiry(&self, account: &AccountId) -> Option<DateTime<Utc>> {
        let state = self.inner.state.lock();
        state
            .sessions
            .get(account)?
            .held
            .as_ref()
            .map(|held| held.session.expires_at)
    }

    fn drop_expired(&self, account: &AccountId) {
        let mut state = self.inner.state.lock();
        if let Some(slot) = state.sessions.get_mut(account)
            && slot
                .held
                .as_ref()
                .is_some_and(|held| held.session.expires_at <= Utc::now())
        {
            slot.held = None;
        }
    }

    fn renewal_ended(&self, account: &AccountId, generation: u64) {
        let mut state = self.inner.state.lock();
        if let Some(slot) = state.sessions.get_mut(account)
            && slot
                .renewal
                .as_ref()
                .is_some_and(|renewal| renewal.generation == generation)
        {
            slot.renewal = None;
        }
    }
}

async fn renewal_loop(
    inner: Weak<Inner>,
    account: AccountId,
    generation: u64,
    cancel: CancellationToken,
) {
    let first = inner
        .upgrade()
        .and_then(|inner| Accounts { inner }.renewal_plan(&account));
    let Some(mut plan) = first else {
        return;
    };
    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            () = tokio::time::sleep(plan.wait) => {},
        }
        let Some(inner) = inner.upgrade() else {
            return;
        };
        match (Accounts { inner })
            .renewal_step(&account, plan.planned_for)
            .await
        {
            Some(next) => plan = next,
            None => break,
        }
    }
    if let Some(inner) = inner.upgrade() {
        Accounts { inner }.renewal_ended(&account, generation);
    }
}
