//! Session tokens: minted from the sealed PAT, held in memory only, renewed in
//! the background and handed to the client crate through [`CredentialSource`],
//! which it holds as `dyn`, hence `#[async_trait]` on the implementation.
//!
//! Concurrent callers share one mint per account: a caller asking for a fresh
//! token gets a new one unless another caller minted one while it waited. A
//! token the instance refuses after a fresh mint signs the account out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sp_client::{CredentialError, CredentialSource, Freshness, JwtSession};
use sp_protocol::Jwt;
use sp_protocol::ids::AccountId;
use sp_secure::SealedString;
use tokio::sync::broadcast;

use crate::accounts::Accounts;
use crate::renewal::Renewal;
use crate::schedule::satisfies;
use crate::state::State;

/// A session token was minted for an account. It names the account only; pull
/// the token with [`Accounts::session`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renewed {
    pub account: AccountId,
}

#[derive(Debug, Default)]
pub(crate) struct SessionSlot {
    pub(crate) held: Option<HeldSession>,
    pub(crate) generation: u64,
    pub(crate) renewal: Option<Renewal>,
}

#[derive(Debug)]
pub(crate) struct HeldSession {
    pub(crate) session: JwtSession,
    pub(crate) minted_at: DateTime<Utc>,
    pub(crate) credential: SealedString,
}

enum Held {
    Ready(JwtSession),
    Mint(u64),
}

impl Accounts {
    pub async fn session(
        &self,
        account: &AccountId,
        freshness: Freshness,
    ) -> Result<JwtSession, CredentialError> {
        let observed = match self.held(account, freshness)? {
            Held::Ready(session) => return Ok(session),
            Held::Mint(generation) => generation,
        };
        let lock = self.mint_lock(account);
        let _minting = lock.lock().await;
        if let Some(session) = self.minted_since(account, observed, freshness) {
            return Ok(session);
        }
        self.mint(account).await
    }

    #[must_use]
    pub fn credentials(&self, account: &AccountId) -> Arc<dyn CredentialSource> {
        Arc::new(AccountCredentials {
            accounts: self.clone(),
            account: account.clone(),
        })
    }

    #[must_use]
    pub fn renewals(&self) -> broadcast::Receiver<Renewed> {
        self.inner.renewed.subscribe()
    }

    pub(crate) fn install_session(
        &self,
        account: &AccountId,
        session: JwtSession,
        credential: SealedString,
    ) -> Result<JwtSession, CredentialError> {
        {
            let mut guard = self.inner.state.lock();
            let state = &mut *guard;
            if state.current_credential(account).as_ref() != Some(&credential) {
                return Err(CredentialError::SignedOut);
            }
            let sequence = state.next_sequence();
            let slot = state.sessions.entry(account.clone()).or_default();
            slot.generation = slot.generation.wrapping_add(1);
            let held = HeldSession {
                session: session.clone(),
                minted_at: Utc::now(),
                credential,
            };
            slot.held = Some(held);
            if !slot.renewal.as_ref().is_some_and(Renewal::is_running) {
                slot.renewal = Some(self.start_renewal(account, sequence));
            }
            drop(guard);
        }
        // Why: discard-ok: a send fails only while nobody subscribes to renewals.
        self.inner
            .renewed
            .send(Renewed {
                account: account.clone(),
            })
            .ok();
        tracing::debug!(
            account = %account,
            expires_at = %session.expires_at,
            "session token minted"
        );
        Ok(session)
    }

    pub(crate) fn drop_session(&self, account: &AccountId) {
        let removed = self.inner.state.lock().sessions.remove(account);
        if let Some(renewal) = removed.and_then(|slot| slot.renewal) {
            renewal.stop();
        }
    }

    pub(crate) fn mint_lock(&self, account: &AccountId) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(
            self.inner
                .mint_locks
                .lock()
                .entry(account.clone())
                .or_default(),
        )
    }

    fn held(&self, account: &AccountId, freshness: Freshness) -> Result<Held, CredentialError> {
        held_in(&self.inner.state.lock(), account, freshness)
    }

    fn minted_since(
        &self,
        account: &AccountId,
        observed: u64,
        freshness: Freshness,
    ) -> Option<JwtSession> {
        let wanted = match freshness {
            Freshness::Fresh => Freshness::Any,
            other => other,
        };
        minted_in(&self.inner.state.lock(), account, observed, wanted)
    }
}

fn held_in(
    state: &State,
    account: &AccountId,
    freshness: Freshness,
) -> Result<Held, CredentialError> {
    let credential = state
        .current_credential(account)
        .ok_or(CredentialError::SignedOut)?;
    let slot = state.sessions.get(account);
    if let Some(held) = slot.and_then(|slot| slot.held.as_ref())
        && held.credential == credential
        && satisfies(held.session.expires_at, Utc::now(), freshness)
    {
        return Ok(Held::Ready(held.session.clone()));
    }
    Ok(Held::Mint(slot.map_or(0, |slot| slot.generation)))
}

fn minted_in(
    state: &State,
    account: &AccountId,
    observed: u64,
    wanted: Freshness,
) -> Option<JwtSession> {
    let credential = state.current_credential(account)?;
    let slot = state.sessions.get(account)?;
    let held = slot.held.as_ref()?;
    let usable = slot.generation != observed
        && held.credential == credential
        && satisfies(held.session.expires_at, Utc::now(), wanted);
    usable.then(|| held.session.clone())
}

struct AccountCredentials {
    accounts: Accounts,
    account: AccountId,
}

#[async_trait]
impl CredentialSource for AccountCredentials {
    async fn jwt(&self, freshness: Freshness) -> Result<Jwt, CredentialError> {
        self.accounts
            .session(&self.account, freshness)
            .await
            .map(|session| session.jwt)
    }

    async fn rejected(&self) {
        tracing::warn!(
            account = %self.account,
            "the instance refused a fresh session token; signing out"
        );
        if let Err(error) = self.accounts.sign_out(&self.account) {
            tracing::warn!(
                account = %self.account,
                error = %error,
                "signing the account out failed"
            );
        }
    }
}
